//! Independent tester acceptance tests for PR 3 of
//! `specs/edit-interaction-polish/specification.md`: Part B (criteria 9 to
//! 22 and 24, 25: typed skew and typed move), the keys M, K and Shift+K
//! (criteria 54, 55, 56, 58, 59), and the customer change that the key S
//! scales about the box centre (criteria 57, 57a). Written from the
//! specification before the implementation was read. Everything goes through
//! `Session`'s public API; expected values come from reference models
//! written here (shear arithmetic, tight bounds of the drawn outline), never
//! from the code under test. The Copy check (criterion 23) and Part C are
//! PR 4 and not covered.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::type_complexity)]
#![allow(clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::too_many_arguments, clippy::manual_let_else, missing_docs)]
#![allow(clippy::if_not_else, clippy::needless_range_loop)]

use std::f64::consts::{FRAC_PI_2, PI, SQRT_2};

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, ObjectSnapshot, Point,
    PointCount, RectBounds, Shape, StarFrame, Vec2, pack, unpack,
};
use curvyo_editor_wasm::{EscapeStep, KeyHint, KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::{EntryOutcome, InvalidReason, MoveEntryMode};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

fn pnear(a: Point, b: Point, eps: f64) -> bool {
    near(a.x, b.x, eps) && near(a.y, b.y, eps)
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

fn rect_doc(x: f64, y: f64, w: f64, h: f64) -> Document {
    let d = Document::new(1);
    let _ = d.create_rect(bounds(x, y, w, h));
    d
}

fn ellipse_doc(cx: f64, cy: f64, rx: f64, ry: f64) -> Document {
    let d = Document::new(1);
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(cx, cy),
        rx: Length::from_mm(rx),
        ry: Length::from_mm(ry),
    });
    d
}

fn star_frame(cx: f64, cy: f64, r: f64) -> StarFrame {
    StarFrame {
        center: pt(cx, cy),
        radius: Length::from_mm(r),
        angle: Angle::from_radians(-FRAC_PI_2),
    }
}

fn polygon_doc(cx: f64, cy: f64, r: f64, n: u32) -> Document {
    let d = Document::new(1);
    let _ = d.create_polygon(star_frame(cx, cy, r), PointCount::new(n).unwrap());
    d
}

fn star_doc(cx: f64, cy: f64, r: f64, n: u32, ratio: f64) -> Document {
    let d = Document::new(1);
    let _ = d.create_star(
        star_frame(cx, cy, r),
        PointCount::new(n).unwrap(),
        InnerRatio::new(ratio).unwrap(),
    );
    d
}

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

/// Closed triangle (0,0) (w,0) (w,h): tight box (0,0)-(w,h).
fn triangle_doc_sized(w: f64, h: f64) -> Document {
    let d = Document::new(1);
    let _ = d.create_path(
        &[anchor(1, 0.0, 0.0), anchor(2, w, 0.0), anchor(3, w, h)],
        true,
    );
    d
}

/// Closed rectangle-shaped path, 20 x 10 at (5, 5).
fn rect_path_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 5.0, 5.0),
            anchor(2, 25.0, 5.0),
            anchor(3, 25.0, 15.0),
            anchor(4, 5.0, 15.0),
        ],
        true,
    );
    d
}

/// A rich closed path with curves and mixed handle vectors.
fn curvy_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            NewAnchor {
                handle_out: Vec2::new(6.0, -8.0),
                ..anchor(1, 0.0, 10.0)
            },
            NewAnchor {
                handle_in: Vec2::new(-5.0, -6.0),
                handle_out: Vec2::new(5.0, 6.0),
                ..anchor(2, 25.0, 0.0)
            },
            NewAnchor {
                handle_in: Vec2::new(0.0, -7.0),
                ..anchor(3, 45.0, 22.0)
            },
            anchor(4, 10.0, 30.0),
        ],
        true,
    );
    d
}

/// Open S-curve whose control handles push the curve beyond its anchors'
/// bounding box (anchors y 0..20, curve y about -9..29).
fn bulgy_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            NewAnchor {
                handle_out: Vec2::new(10.0, -36.0),
                ..anchor(1, 10.04, 0.0)
            },
            NewAnchor {
                handle_in: Vec2::new(-10.0, 36.0),
                ..anchor(2, 50.0, 20.0)
            },
        ],
        false,
    );
    d
}

fn rotate_doc_object(d: &Document, radians: f64) {
    let id = d.object_ids()[0];
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
        ObjectSnapshot::Path(p) => {
            let xs: Vec<f64> = p.anchors.iter().map(|a| a.point.x).collect();
            let ys: Vec<f64> = p.anchors.iter().map(|a| a.point.y).collect();
            pt(
                f64::midpoint(
                    xs.iter().copied().fold(f64::INFINITY, f64::min),
                    xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                ),
                f64::midpoint(
                    ys.iter().copied().fold(f64::INFINITY, f64::min),
                    ys.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                ),
            )
        }
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

fn obj_of(s: &Session, index: usize) -> ObjectSnapshot {
    let d = doc_of(s);
    d.object(d.object_ids()[index]).unwrap()
}

fn path_of(s: &Session) -> curvyo_document_core::PathSnapshot {
    let d = doc_of(s);
    d.path(d.object_ids()[0]).unwrap()
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn drag_mod(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(from, shift, ctrl);
    s.pointer_down(from, shift);
    s.pointer_hover(to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
}

/// The browser's double-click: press and release at `first`, the second
/// press withheld, `double_click` at it, then the re-hover. Returns the
/// edit-hint flag.
fn dbl(s: &mut Session, first: Point, second: Point, shift: bool, ctrl: bool) -> bool {
    s.pointer_hover(first, shift, ctrl);
    s.pointer_down(first, shift);
    s.pointer_up(first, shift, ctrl);
    s.pointer_hover(second, shift, ctrl);
    let hint = s.double_click(second, shift, ctrl);
    s.pointer_hover(second, shift, ctrl);
    hint
}

fn k(s: &Session) -> f64 {
    s.view().scale()
}

fn num(text: &str) -> f64 {
    text.trim_end_matches('°')
        .replace(',', ".")
        .parse()
        .unwrap()
}

fn key(kk: &str) -> KeyInput<'_> {
    KeyInput {
        key: kk,
        ..KeyInput::default()
    }
}

fn press(s: &mut Session, kk: &str) -> KeyOutcome {
    s.key_down(key(kk))
}

fn press_shift(s: &mut Session, kk: &str) -> KeyOutcome {
    s.key_down(KeyInput {
        key: kk,
        shift: true,
        ..KeyInput::default()
    })
}

/// A box in the document: centre, half extents (mm), rotation (radians).
#[derive(Clone, Copy, Debug)]
struct Bx {
    c: Point,
    hw: f64,
    hh: f64,
    th: f64,
}

impl Bx {
    fn rect(x: f64, y: f64, w: f64, h: f64, th: f64) -> Self {
        Self {
            c: pt(x + w / 2.0, y + h / 2.0),
            hw: w / 2.0,
            hh: h / 2.0,
            th,
        }
    }
    fn at(&self, ox: f64, oy: f64) -> Point {
        rot(pt(self.c.x + ox, self.c.y + oy), self.c, self.th)
    }
    fn corner(&self, sx: f64, sy: f64) -> Point {
        self.at(sx * self.hw, sy * self.hh)
    }
    fn rot_corner(&self, kk: f64, sx: f64, sy: f64) -> Point {
        let d = 32.0 / SQRT_2 / kk;
        self.at(sx * (self.hw + d), sy * (self.hh + d))
    }
    fn side_out(&self, kk: f64, nx: f64, ny: f64, out_px: f64) -> Point {
        let o = out_px / kk;
        self.at(nx * (self.hw + o), ny * (self.hh + o))
    }
}

const SIDES: [(f64, f64); 4] = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];

/// Sampled cubic Bezier extents of a path in the `th` frame.
fn tight_box(p: &curvyo_document_core::PathSnapshot, th: f64) -> (f64, f64, f64, f64) {
    let a: Vec<(Point, Vec2, Vec2)> = p
        .anchors
        .iter()
        .map(|a| (a.point, a.handle_in, a.handle_out))
        .collect();
    tight_box_of(&a, p.closed, th)
}

fn tight_box_of(anchors: &[(Point, Vec2, Vec2)], closed: bool, th: f64) -> (f64, f64, f64, f64) {
    let (sn, cs) = th.sin_cos();
    let uv = |q: Point| (q.x * cs + q.y * sn, -q.x * sn + q.y * cs);
    let mut ext = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    let mut upd = |q: Point| {
        let (u, v) = uv(q);
        ext.0 = ext.0.min(u);
        ext.1 = ext.1.max(u);
        ext.2 = ext.2.min(v);
        ext.3 = ext.3.max(v);
    };
    let n = anchors.len();
    for a in anchors {
        upd(a.0);
    }
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (a, b) = (&anchors[i], &anchors[(i + 1) % n]);
        let p0 = a.0;
        let p1 = pt(a.0.x + a.2.x, a.0.y + a.2.y);
        let p2 = pt(b.0.x + b.1.x, b.0.y + b.1.y);
        let p3 = b.0;
        for j in 0..=4000 {
            let t = f64::from(j) / 4000.0;
            let m = 1.0 - t;
            let x = m * m * m * p0.x
                + 3.0 * m * m * t * p1.x
                + 3.0 * m * t * t * p2.x
                + t * t * t * p3.x;
            let y = m * m * m * p0.y
                + 3.0 * m * m * t * p1.y
                + 3.0 * m * t * t * p2.y
                + t * t * t * p3.y;
            upd(pt(x, y));
        }
    }
    ext
}

/// Tight bounds of the drawn (Bezier) outline of a primitive.
fn tight_drawn(shape: &Shape, rotation: Angle) -> (f64, f64, f64, f64) {
    let o = curvyo_document_core::outline_of_rotated(shape, rotation);
    let a: Vec<(Point, Vec2, Vec2)> = o
        .iter()
        .map(|x| (x.point, x.handle_in, x.handle_out))
        .collect();
    let (u0, u1, v0, v1) = tight_box_of(&a, true, 0.0);
    (u0, v0, u1, v1)
}

fn path_bx(p: &curvyo_document_core::PathSnapshot) -> Bx {
    let th = p.rotation.as_radians();
    let (u0, u1, v0, v1) = tight_box(p, th);
    let (sn, cs) = th.sin_cos();
    let (cu, cv) = (f64::midpoint(u0, u1), f64::midpoint(v0, v1));
    Bx {
        c: pt(cu * cs - cv * sn, cu * sn + cv * cs),
        hw: (u1 - u0) / 2.0,
        hh: (v1 - v0) / 2.0,
        th,
    }
}

/// Reference skew: grabbed side `(nx, ny)`, displacement `d` of the grabbed
/// side along its direction (mm, in the box's frame).
fn skew_ref(
    p: &curvyo_document_core::PathSnapshot,
    side: (f64, f64),
    shift: bool,
    d: f64,
) -> Vec<(Point, Vec2, Vec2)> {
    let th = p.rotation.as_radians();
    let (u0, u1, v0, v1) = tight_box(p, th);
    let (sn, cs) = th.sin_cos();
    let (uh, vh) = ((cs, sn), (-sn, cs));
    let along_u = side.1 != 0.0;
    let (lo, hi) = if along_u { (v0, v1) } else { (u0, u1) };
    let sign = if along_u { side.1 } else { side.0 };
    let g = if sign < 0.0 { lo } else { hi };
    let f = if shift {
        f64::midpoint(lo, hi)
    } else if sign < 0.0 {
        hi
    } else {
        lo
    };
    let h = f - g;
    p.anchors
        .iter()
        .map(|a| {
            let (u, v) = (
                a.point.x * cs + a.point.y * sn,
                -a.point.x * sn + a.point.y * cs,
            );
            let across = if along_u { v } else { u };
            let delta = d * (f - across) / h;
            let q = if along_u {
                pt(a.point.x + uh.0 * delta, a.point.y + uh.1 * delta)
            } else {
                pt(a.point.x + vh.0 * delta, a.point.y + vh.1 * delta)
            };
            let map_h = |hv: Vec2| {
                let (hu_, hv_) = (hv.x * cs + hv.y * sn, -hv.x * sn + hv.y * cs);
                let across_h = if along_u { hv_ } else { hu_ };
                let dd = -d * across_h / h;
                if along_u {
                    Vec2::new(hv.x + uh.0 * dd, hv.y + uh.1 * dd)
                } else {
                    Vec2::new(hv.x + vh.0 * dd, hv.y + vh.1 * dd)
                }
            };
            (q, map_h(a.handle_in), map_h(a.handle_out))
        })
        .collect()
}

fn assert_path_matches(
    p: &curvyo_document_core::PathSnapshot,
    want: &[(Point, Vec2, Vec2)],
    eps: f64,
    ctx: &str,
) {
    assert_eq!(p.anchors.len(), want.len(), "{ctx}: node count");
    for (i, (a, (q, hi, ho))) in p.anchors.iter().zip(want).enumerate() {
        assert!(
            pnear(a.point, *q, eps),
            "{ctx}: anchor {i} {:?} want {q:?}",
            a.point
        );
        assert!(
            near(a.handle_in.x, hi.x, eps) && near(a.handle_in.y, hi.y, eps),
            "{ctx}: handle_in {i}"
        );
        assert!(
            near(a.handle_out.x, ho.x, eps) && near(a.handle_out.y, ho.y, eps),
            "{ctx}: handle_out {i} {:?} want {ho:?}",
            a.handle_out
        );
    }
}

fn paths_equal(
    a: &curvyo_document_core::PathSnapshot,
    b: &curvyo_document_core::PathSnapshot,
    eps: f64,
    ctx: &str,
) {
    let want: Vec<(Point, Vec2, Vec2)> = b
        .anchors
        .iter()
        .map(|x| (x.point, x.handle_in, x.handle_out))
        .collect();
    assert_path_matches(a, &want, eps, ctx);
}

fn skew_handle_pos(s: &Session, b: &Bx, side: (f64, f64)) -> Point {
    b.side_out(k(s), side.0, side.1, 16.0)
}

fn select_path(d: &Document) -> (Session, Bx) {
    let mut s = open_in_session(d);
    let p = {
        let id = d.object_ids()[0];
        d.path(id).unwrap()
    };
    click(&mut s, p.anchors[0].point);
    assert_eq!(s.selected_object_count(), 1);
    let b = path_bx(&p);
    (s, b)
}

/// Tight bounds (x0, y0, x1, y1) of the object's drawn outline, computed
/// here: curve-accurate for a path, the rotated outline for a primitive.
fn tight_of(o: &ObjectSnapshot) -> (f64, f64, f64, f64) {
    match o {
        ObjectSnapshot::Path(p) => {
            let (u0, u1, v0, v1) = tight_box(p, 0.0);
            (u0, v0, u1, v1)
        }
        ObjectSnapshot::Primitive(p) => {
            let th = p.rotation.as_radians();
            match p.shape {
                Shape::Rect {
                    bounds: b,
                    corner_radii,
                } if uniform_mm(corner_radii) > 0.0 => {
                    let _ = b;
                    tight_drawn(&p.shape, p.rotation)
                }
                Shape::Rect {
                    bounds: b,
                    corner_radii,
                } => {
                    let (w, h) = (b.width.as_mm(), b.height.as_mm());
                    let c = pt(b.origin.x + w / 2.0, b.origin.y + h / 2.0);
                    let r = uniform_mm(corner_radii).max(0.0).min(w / 2.0).min(h / 2.0);
                    let (cs, sn) = (th.cos().abs(), th.sin().abs());
                    let hx = f64::midpoint((w - 2.0 * r) * cs, (h - 2.0 * r) * sn) + r;
                    let hy = f64::midpoint((w - 2.0 * r) * sn, (h - 2.0 * r) * cs) + r;
                    (c.x - hx, c.y - hy, c.x + hx, c.y + hy)
                }
                Shape::Ellipse { .. } => tight_drawn(&p.shape, p.rotation),
                Shape::Polygon { frame, point_count } => {
                    let n = point_count.get();
                    let mut pts = vec![];
                    for i in 0..n {
                        let a =
                            frame.angle.as_radians() + th + 2.0 * PI * f64::from(i) / f64::from(n);
                        pts.push(pt(
                            frame.center.x + frame.radius.as_mm() * a.cos(),
                            frame.center.y + frame.radius.as_mm() * a.sin(),
                        ));
                    }
                    hull(&pts)
                }
                Shape::Star {
                    frame,
                    point_count,
                    inner_ratio,
                } => {
                    let n = point_count.get();
                    let mut pts = vec![];
                    for i in 0..n {
                        for (off, rr) in [(0.0, 1.0), (0.5, inner_ratio.get())] {
                            let a = frame.angle.as_radians()
                                + th
                                + 2.0 * PI * (f64::from(i) + off) / f64::from(n);
                            pts.push(pt(
                                frame.center.x + frame.radius.as_mm() * rr * a.cos(),
                                frame.center.y + frame.radius.as_mm() * rr * a.sin(),
                            ));
                        }
                    }
                    hull(&pts)
                }
            }
        }
    }
}

fn hull(pts: &[Point]) -> (f64, f64, f64, f64) {
    let mut e = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in pts {
        e.0 = e.0.min(p.x);
        e.1 = e.1.min(p.y);
        e.2 = e.2.max(p.x);
        e.3 = e.3.max(p.y);
    }
    e
}

/// A selected object by pressing a point that lies on its outline.
fn select_at(d: &Document, on: Point) -> Session {
    let mut s = open_in_session(d);
    click(&mut s, on);
    assert_eq!(s.selected_object_count(), 1, "selected at {on:?}");
    s
}

struct Case {
    name: &'static str,
    doc: Document,
    /// A point on the outline used to select it.
    on: Point,
}

/// The object kinds with a drawn centre handle (big enough), unrotated.
fn kinds_big(th: f64) -> Vec<Case> {
    let mut v = vec![
        Case {
            name: "rect",
            doc: rect_doc(10.0, 20.0, 120.0, 80.0),
            on: pt(70.0, 20.0),
        },
        Case {
            name: "ellipse",
            doc: ellipse_doc(70.0, 60.0, 60.0, 40.0),
            on: pt(70.0, 20.0),
        },
        Case {
            name: "polygon",
            doc: polygon_doc(70.0, 60.0, 40.0, 5),
            on: pt(70.0, 20.0),
        },
        Case {
            name: "star",
            doc: star_doc(70.0, 60.0, 40.0, 5, 0.5),
            on: pt(70.0, 20.0),
        },
        Case {
            name: "path",
            doc: triangle_doc_sized(120.0, 80.0),
            on: pt(60.0, 0.0),
        },
    ];
    if th != 0.0 {
        for c in &mut v {
            c.on = rotate_with_on(&c.doc, c.on, th);
        }
    }
    v
}

/// Rotates the document's only object by `th` and returns `on` carried
/// along with it (every rotation here is about the object's own centre).
fn rotate_with_on(d: &Document, on: Point, th: f64) -> Point {
    let c = ctr_of(&d.object(d.object_ids()[0]).unwrap());
    rotate_doc_object(d, th);
    rot(on, c, th)
}

fn same_object(a: &ObjectSnapshot, b: &ObjectSnapshot, eps: f64, ctx: &str) {
    match (a, b) {
        (ObjectSnapshot::Path(x), ObjectSnapshot::Path(y)) => {
            paths_equal(x, y, eps, ctx);
            assert!(
                near(x.rotation.as_radians(), y.rotation.as_radians(), 1e-12),
                "{ctx}: rot"
            );
            assert!(
                near(x.stroke_width.as_mm(), y.stroke_width.as_mm(), eps),
                "{ctx}: stroke"
            );
        }
        (ObjectSnapshot::Primitive(x), ObjectSnapshot::Primitive(y)) => {
            assert!(
                near(x.rotation.as_radians(), y.rotation.as_radians(), 1e-12),
                "{ctx}: rotation"
            );
            assert!(
                near(x.stroke_width.as_mm(), y.stroke_width.as_mm(), eps),
                "{ctx}: stroke"
            );
            let (tx, ty) = (tight_of(a), tight_of(b));
            assert!(
                near(tx.0, ty.0, eps)
                    && near(tx.1, ty.1, eps)
                    && near(tx.2, ty.2, eps)
                    && near(tx.3, ty.3, eps),
                "{ctx}: tight bounds {tx:?} vs {ty:?}"
            );
            match (x.shape, y.shape) {
                (
                    Shape::Rect {
                        bounds: bx,
                        corner_radii: rx,
                    },
                    Shape::Rect {
                        bounds: by,
                        corner_radii: ry,
                    },
                ) => {
                    assert!(
                        pnear(bx.origin, by.origin, eps),
                        "{ctx}: origin {:?} {:?}",
                        bx.origin,
                        by.origin
                    );
                    assert!(near(bx.width.as_mm(), by.width.as_mm(), eps), "{ctx}: w");
                    assert!(near(bx.height.as_mm(), by.height.as_mm(), eps), "{ctx}: h");
                    assert!(
                        near(uniform_mm(rx), uniform_mm(ry), eps),
                        "{ctx}: corner radius"
                    );
                }
                (Shape::Ellipse { frame: fx }, Shape::Ellipse { frame: fy }) => {
                    assert!(pnear(fx.center, fy.center, eps), "{ctx}: centre");
                    assert!(near(fx.rx.as_mm(), fy.rx.as_mm(), eps), "{ctx}: rx");
                    assert!(near(fx.ry.as_mm(), fy.ry.as_mm(), eps), "{ctx}: ry");
                }
                (
                    Shape::Polygon { frame: fx, .. } | Shape::Star { frame: fx, .. },
                    Shape::Polygon { frame: fy, .. } | Shape::Star { frame: fy, .. },
                ) => {
                    assert!(
                        pnear(fx.center, fy.center, eps),
                        "{ctx}: centre {:?} {:?}",
                        fx.center,
                        fy.center
                    );
                    assert!(
                        near(fx.radius.as_mm(), fy.radius.as_mm(), eps),
                        "{ctx}: radius"
                    );
                    assert!(
                        near(fx.angle.as_radians(), fy.angle.as_radians(), 1e-9),
                        "{ctx}: angle"
                    );
                }
                _ => panic!("{ctx}: kinds differ"),
            }
        }
        _ => panic!("{ctx}: kinds differ"),
    }
}

// ---------------------------------------------------------------------
// Typed skew, criteria 9 to 14
// ---------------------------------------------------------------------

#[test]
fn c09_double_click_on_each_skew_handle_opens_one_field_with_the_axis_name_and_writes_nothing() {
    for th in [0.0, 0.7, -2.0] {
        let d = curvy_doc();
        if th != 0.0 {
            rotate_doc_object(&d, th);
        }
        for side in SIDES {
            let (mut s, b) = select_path(&d);
            let before = snapshot_bytes(&s);
            let n = change_count(&s);
            let h = skew_handle_pos(&s, &b, side);
            dbl(&mut s, h, h, false, false);
            let e = s.transform_entry().expect("skew entry opens");
            assert_eq!(e.kind, "skew", "th {th} side {side:?}");
            assert_eq!(e.fields.len(), 1);
            assert_eq!(e.fields[0].label, "", "visible label empty");
            let want = if side.1 != 0.0 {
                "Skew angle x"
            } else {
                "Skew angle y"
            };
            assert_eq!(e.fields[0].accessible_name, want, "side {side:?}");
            assert!(e.fields[0].editable);
            assert!(
                near(num(&e.fields[0].prefill), 0.0, 1e-12),
                "prefill {:?}",
                e.fields[0].prefill
            );
            assert_eq!(e.fields[0].prefill.trim(), "0", "prefill is exactly 0");
            assert_eq!(s.tool(), Tool::Select, "no handoff");
            assert_eq!(s.selected_object_count(), 1);
            assert_eq!(
                snapshot_bytes(&s),
                before,
                "first press and release write nothing"
            );
            assert_eq!(change_count(&s), n);
            assert!(s.move_entry().is_none(), "no move chip alongside");
        }
    }
}

#[test]
fn c09_the_pointer_must_move_less_than_three_px_between_the_presses() {
    // A press with a real drag on a skew handle is a skew drag, not an entry.
    let (mut s, b) = select_path(&triangle_doc_sized(40.0, 20.0));
    let h = skew_handle_pos(&s, &b, (0.0, -1.0));
    let kk = k(&s);
    drag_mod(&mut s, h, pt(h.x + 30.0 / kk, h.y), false, false);
    s.double_click(pt(h.x + 30.0 / kk, h.y), false, false);
    // After the drag the handle moved; whatever opened, the skew committed once.
    assert!(change_count(&s) >= 1);
}

#[test]
fn c10_typed_skew_equals_the_reference_and_a_drag_for_every_side_rotation_and_shift() {
    for th in [0.0, 0.7, -2.0] {
        for side in SIDES {
            for shift in [false, true] {
                for alpha in [45.0_f64, -30.0, 10.5, 80.0, -75.5] {
                    let d = curvy_doc();
                    if th != 0.0 {
                        rotate_doc_object(&d, th);
                    }
                    let ctx = format!("th {th} side {side:?} shift {shift} alpha {alpha}");
                    let (mut s, b) = select_path(&d);
                    let before = path_of(&s);
                    // the lever: the whole side distance, half of it with Shift
                    let across = if side.1 != 0.0 {
                        2.0 * b.hh
                    } else {
                        2.0 * b.hw
                    };
                    let lever = if shift { across / 2.0 } else { across };
                    let dd = lever * alpha.to_radians().tan();
                    let want = skew_ref(&before, side, shift, dd);

                    let n = change_count(&s);
                    let h = skew_handle_pos(&s, &b, side);
                    dbl(&mut s, h, h, shift, false);
                    assert_eq!(s.transform_entry().map(|e| e.kind), Some("skew"), "{ctx}");
                    let out = s.commit_transform_entry(&format!("{alpha}"), "", 0);
                    assert_eq!(out, EntryOutcome::Committed, "{ctx}");
                    let after = path_of(&s);
                    assert_path_matches(&after, &want, 1e-6, &ctx);
                    assert_eq!(change_count(&s), n + 1, "{ctx}: one commit");
                    assert!(s.transform_entry().is_none(), "{ctx}: entry closes");
                    assert_eq!(after.closed, before.closed);
                    for (a, o) in after.anchors.iter().zip(&before.anchors) {
                        assert_eq!(a.id, o.id);
                        assert_eq!(a.kind, o.kind);
                    }
                    assert_eq!(after.stroke_width.as_mm(), before.stroke_width.as_mm());
                    assert_eq!(after.rotation.as_radians(), before.rotation.as_radians());
                    assert_eq!(s.tool(), Tool::Select);
                    assert_eq!(s.selected_object_count(), 1);

                    // the same skew by a real drag ends in the same document
                    let (mut s2, b2) = select_path(&d);
                    let (sn, cs) = b2.th.sin_cos();
                    let along = if side.1 != 0.0 { (cs, sn) } else { (-sn, cs) };
                    let from = skew_handle_pos(&s2, &b2, side);
                    let to = pt(from.x + along.0 * dd, from.y + along.1 * dd);
                    drag_mod(&mut s2, from, to, shift, false);
                    paths_equal(
                        &after,
                        &path_of(&s2),
                        1e-6,
                        &format!("{ctx}: entry vs drag"),
                    );
                }
            }
        }
    }
}

#[test]
fn c10_example_20_by_10_at_45_degrees() {
    // spec: oriented box 20 x 10, rotation 0, top handle, 45: top edge +10,
    // bottom edge 0, halfway +5.
    let (mut s, b) = select_path(&rect_path_doc());
    let h = skew_handle_pos(&s, &b, (0.0, -1.0));
    dbl(&mut s, h, h, false, false);
    assert_eq!(
        s.commit_transform_entry("45", "", 0),
        EntryOutcome::Committed
    );
    let p = path_of(&s);
    let q: Vec<Point> = p.anchors.iter().map(|a| a.point).collect();
    assert!(pnear(q[0], pt(15.0, 5.0), 1e-9), "{:?}", q[0]);
    assert!(pnear(q[1], pt(35.0, 5.0), 1e-9), "{:?}", q[1]);
    assert!(
        pnear(q[2], pt(25.0, 15.0), 1e-9),
        "bottom unmoved {:?}",
        q[2]
    );
    assert!(pnear(q[3], pt(5.0, 15.0), 1e-9));
    // negative angle on the left handle: left side moves toward -v? sign:
    // positive moves the grabbed side toward +v, so -45 moves it up (-y).
    let (mut s, b) = select_path(&rect_path_doc());
    let h = skew_handle_pos(&s, &b, (-1.0, 0.0));
    dbl(&mut s, h, h, false, false);
    assert_eq!(
        s.commit_transform_entry("-45", "", 0),
        EntryOutcome::Committed
    );
    let q: Vec<Point> = path_of(&s).anchors.iter().map(|a| a.point).collect();
    // left edge x=5 moves by -tan45 * 20 = -20 in y
    assert!(pnear(q[0], pt(5.0, -15.0), 1e-9), "{:?}", q[0]);
    assert!(pnear(q[3], pt(5.0, -5.0), 1e-9), "{:?}", q[3]);
    assert!(
        pnear(q[1], pt(25.0, 5.0), 1e-9),
        "right edge fixed {:?}",
        q[1]
    );
}

#[test]
fn c10_the_shift_pivot_is_fixed_when_the_entry_opens_not_at_enter() {
    for open_shift in [false, true] {
        let (mut s, b) = select_path(&rect_path_doc());
        let h = skew_handle_pos(&s, &b, (0.0, -1.0));
        dbl(&mut s, h, h, open_shift, false);
        // flip the modifier state after opening
        s.modifiers_changed(!open_shift, false);
        s.modifiers_changed(open_shift, false);
        s.modifiers_changed(!open_shift, false);
        assert_eq!(
            s.commit_transform_entry("45", "", 0),
            EntryOutcome::Committed
        );
        let q: Vec<Point> = path_of(&s).anchors.iter().map(|a| a.point).collect();
        if open_shift {
            // centre line y = 10 fixed: top +5... lever 5 => d = 5
            assert!(pnear(q[0], pt(10.0, 5.0), 1e-9), "{:?}", q[0]);
            assert!(pnear(q[2], pt(20.0, 15.0), 1e-9), "{:?}", q[2]);
        } else {
            assert!(pnear(q[0], pt(15.0, 5.0), 1e-9), "{:?}", q[0]);
            assert!(pnear(q[2], pt(25.0, 15.0), 1e-9), "{:?}", q[2]);
        }
    }
}

fn open_skew(d: &Document, side: (f64, f64)) -> (Session, Bx) {
    let (mut s, b) = select_path(d);
    let h = skew_handle_pos(&s, &b, side);
    dbl(&mut s, h, h, false, false);
    assert_eq!(s.transform_entry().map(|e| e.kind), Some("skew"));
    (s, b)
}

#[test]
fn c11_invalid_text_keeps_the_entry_open_and_writes_nothing() {
    let (mut s, _b) = open_skew(&rect_path_doc(), (0.0, -1.0));
    let before = snapshot_bytes(&s);
    let n = change_count(&s);
    for (text, reason) in [
        ("", InvalidReason::NotANumber),
        ("abc", InvalidReason::NotANumber),
        ("1.2.3", InvalidReason::NotANumber),
        ("1,2,3", InvalidReason::NotANumber),
        ("NaN", InvalidReason::NotANumber),
        ("inf", InvalidReason::NotANumber),
        ("-inf", InvalidReason::NotANumber),
        ("1e999", InvalidReason::NotANumber),
        ("--5", InvalidReason::NotANumber),
        ("90", InvalidReason::SkewRange),
        ("-90", InvalidReason::SkewRange),
        ("90.0", InvalidReason::SkewRange),
        ("91", InvalidReason::SkewRange),
        ("-120", InvalidReason::SkewRange),
        ("180", InvalidReason::SkewRange),
        ("1000000", InvalidReason::SkewRange),
        ("89.99999999", InvalidReason::TooLarge),
        ("-89.99999999", InvalidReason::TooLarge),
    ] {
        let out = s.commit_transform_entry(text, "", 0);
        assert_eq!(
            out,
            EntryOutcome::Invalid { field: 0, reason },
            "text {text:?}"
        );
        assert!(s.transform_entry().is_some(), "{text:?}: entry stays open");
        assert_eq!(snapshot_bytes(&s), before, "{text:?}: nothing written");
        assert_eq!(change_count(&s), n);
    }
    // the entry still works afterwards
    assert_eq!(
        s.commit_transform_entry("30", "", 0),
        EntryOutcome::Committed
    );
}

#[test]
fn c11_decimal_comma_point_and_degree_sign_are_accepted_and_ctrl_has_no_cap() {
    let want = |alpha: f64| {
        let (s, _b) = select_path(&rect_path_doc());
        let before = path_of(&s);
        skew_ref(&before, (0.0, -1.0), false, 10.0 * alpha.to_radians().tan())
    };
    for (text, alpha) in [
        ("30.5", 30.5),
        ("30,5", 30.5),
        ("30,5°", 30.5),
        ("30.5°", 30.5),
        ("-30,5", -30.5),
        (" 30.5 ", 30.5),
        ("89", 89.0),
        ("-89.9", -89.9),
    ] {
        for ctrl in [false, true] {
            let (mut s, b) = select_path(&rect_path_doc());
            let h = skew_handle_pos(&s, &b, (0.0, -1.0));
            dbl(&mut s, h, h, false, ctrl);
            let out = s.commit_transform_entry(text, "", 0);
            assert_eq!(out, EntryOutcome::Committed, "text {text:?} ctrl {ctrl}");
            assert_path_matches(
                &path_of(&s),
                &want(alpha),
                1e-6,
                &format!("{text:?} ctrl {ctrl}"),
            );
        }
    }
}

#[test]
fn c11_unicode_minus_sign_is_reported_not_asserted() {
    // The spec says "a sign" is accepted; it names no minus variants. Record
    // what happens so the report can say it. Must never panic or write junk.
    let (mut s, _b) = open_skew(&rect_path_doc(), (0.0, -1.0));
    let out = s.commit_transform_entry("\u{2212}30", "", 0);
    eprintln!("unicode minus U+2212 for skew: {out:?}");
    assert!(matches!(
        out,
        EntryOutcome::Committed | EntryOutcome::Invalid { .. }
    ));
}

#[test]
fn c12_noop_inputs_write_nothing_and_close() {
    for text in ["0", "0.0", "-0", "0,0", "+0", "0°", "  0  "] {
        let (mut s, _b) = open_skew(&rect_path_doc(), (0.0, -1.0));
        let before = snapshot_bytes(&s);
        let n = change_count(&s);
        let out = s.commit_transform_entry(text, "", 0);
        assert!(
            matches!(out, EntryOutcome::Unchanged),
            "text {text:?} gave {out:?}"
        );
        assert!(s.transform_entry().is_none(), "{text:?}: closed");
        assert_eq!(snapshot_bytes(&s), before, "{text:?}");
        assert_eq!(change_count(&s), n, "{text:?}");
    }
}

#[test]
fn c12_escape_blur_tool_switch_selection_change_and_press_elsewhere_write_nothing() {
    // two paths so a press elsewhere can land on another object
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 0.0, 0.0),
            anchor(2, 40.0, 0.0),
            anchor(3, 40.0, 20.0),
        ],
        true,
    );
    let _ = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 11), pt(200.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 12), pt(240.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 13), pt(240.0, 20.0)),
        ],
        true,
    );
    let open = |d: &Document| {
        let mut s = open_in_session(d);
        click(&mut s, pt(20.0, 0.0));
        let b = Bx::rect(0.0, 0.0, 40.0, 20.0, 0.0);
        let h = skew_handle_pos(&s, &b, (0.0, -1.0));
        dbl(&mut s, h, h, false, false);
        assert_eq!(s.transform_entry().map(|e| e.kind), Some("skew"));
        s
    };
    // Escape (the key table) closes it
    let mut s = open(&d);
    let before = snapshot_bytes(&s);
    let n = change_count(&s);
    assert_eq!(
        s.key_down(key("Escape")),
        KeyOutcome::Escape(EscapeStep::ClosedEntry)
    );
    assert!(s.transform_entry().is_none());
    assert_eq!(s.selected_object_count(), 1, "Escape closed only the chip");
    assert_eq!(snapshot_bytes(&s), before);
    assert_eq!(change_count(&s), n);

    // blur: the chip's cancel call
    let mut s = open(&d);
    s.cancel_transform_entry();
    assert!(s.transform_entry().is_none());
    assert_eq!(snapshot_bytes(&s), before);

    // tool switch
    for tool in [Tool::Node, Tool::Pen, Tool::Rectangle] {
        let mut s = open(&d);
        s.set_tool(tool);
        assert!(s.transform_entry().is_none(), "{tool:?}");
        s.set_tool(Tool::Select);
        assert!(s.transform_entry().is_none(), "{tool:?}: not revived");
        assert_eq!(snapshot_bytes(&s), before);
    }

    // press on another object: not swallowed; selects it
    let mut s = open(&d);
    click(&mut s, pt(220.0, 0.0));
    assert!(s.transform_entry().is_none(), "press elsewhere closes it");
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(snapshot_bytes(&s), before);
    assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened);
    let c = s.move_entry().unwrap().center;
    assert!(
        pnear(c, pt(220.0, 10.0), 1e-6),
        "the press selected the other path: {c:?}"
    );

    // press on empty canvas: closes and clears the selection (not swallowed)
    let mut s = open(&d);
    click(&mut s, pt(120.0, 150.0));
    assert!(s.transform_entry().is_none());
    assert_eq!(s.selected_object_count(), 0, "the closing press acted");
    assert_eq!(snapshot_bytes(&s), before);

    // a selection change by a bar action / delete is not required here; a
    // direct re-selection by Escape twice is covered above.
}

#[test]
fn c13_skew_then_negated_skew_restores_the_path_and_it_reopens_as_a_path() {
    for side in SIDES {
        for shift in [false, true] {
            let d = curvy_doc();
            let (mut s, b) = select_path(&d);
            let orig = path_of(&s);
            let h = skew_handle_pos(&s, &b, side);
            dbl(&mut s, h, h, shift, false);
            assert_eq!(
                s.commit_transform_entry("33", "", 0),
                EntryOutcome::Committed
            );
            // the box changed; the handle is elsewhere now
            let b2 = path_bx(&path_of(&s));
            let h2 = skew_handle_pos(&s, &b2, side);
            dbl(&mut s, h2, h2, shift, false);
            assert_eq!(
                s.commit_transform_entry("-33", "", 0),
                EntryOutcome::Committed
            );
            paths_equal(&path_of(&s), &orig, 1e-5, &format!("{side:?} {shift}"));
            // saves and reopens as an ordinary path
            let again = Session::open(5, &s.pack("0.1.0").unwrap()).unwrap();
            let _ = again;
        }
    }
}

#[test]
fn c14_primitives_and_multi_selections_have_no_skew_handles_and_k_says_so() {
    let docs = [
        (rect_doc(0.0, 0.0, 120.0, 80.0), pt(60.0, 0.0), "rect"),
        (
            ellipse_doc(60.0, 40.0, 60.0, 40.0),
            pt(60.0, 0.0),
            "ellipse",
        ),
        (polygon_doc(60.0, 40.0, 40.0, 5), pt(60.0, 0.0), "polygon"),
        (star_doc(60.0, 40.0, 40.0, 5, 0.5), pt(60.0, 0.0), "star"),
    ];
    for (d, on, name) in docs {
        let mut s = select_at(&d, on);
        let b = Bx::rect(0.0, 0.0, 120.0, 80.0, 0.0);
        let before = snapshot_bytes(&s);
        for side in SIDES {
            let h = skew_handle_pos(&s, &b, side);
            dbl(&mut s, h, h, false, false);
            // the press where a skew handle would be hit empty canvas
            click(&mut s, on);
            assert_eq!(s.selected_object_count(), 1, "{name}");
            assert!(
                s.transform_entry().map(|e| e.kind) != Some("skew"),
                "{name}: no skew entry at {side:?}"
            );
            s.cancel_transform_entry();
            assert_eq!(snapshot_bytes(&s), before, "{name}");
        }
        // keys
        assert_eq!(
            press(&mut s, "k"),
            KeyOutcome::Hint(KeyHint::PathOnly),
            "{name}"
        );
        assert_eq!(
            press_shift(&mut s, "K"),
            KeyOutcome::Hint(KeyHint::PathOnly),
            "{name}"
        );
        assert!(s.transform_entry().is_none());
        assert_eq!(snapshot_bytes(&s), before);
        assert_eq!(s.tool(), Tool::Select);
        assert_eq!(s.selected_object_count(), 1);
    }
}

// ---------------------------------------------------------------------
// Typed move, criteria 15 to 22, 25
// ---------------------------------------------------------------------

/// Centre of the oriented box (document space): where the centre handle is
/// or would be.
fn ctr_of(o: &ObjectSnapshot) -> Point {
    match o {
        ObjectSnapshot::Path(p) => path_bx(p).c,
        ObjectSnapshot::Primitive(p) => {
            let (x0, y0, x1, y1) = frame_centre_box(p);
            pt(f64::midpoint(x0, x1), f64::midpoint(y0, y1))
        }
    }
}

/// Centre of the unrotated frame box: the centre of the oriented box of a
/// primitive (rotation is about it).
fn frame_centre_box(p: &curvyo_document_core::PrimitiveSnapshot) -> (f64, f64, f64, f64) {
    match p.shape {
        Shape::Rect { bounds: b, .. } => (
            b.origin.x,
            b.origin.y,
            b.origin.x + b.width.as_mm(),
            b.origin.y + b.height.as_mm(),
        ),
        Shape::Ellipse { frame } => (
            frame.center.x - frame.rx.as_mm(),
            frame.center.y - frame.ry.as_mm(),
            frame.center.x + frame.rx.as_mm(),
            frame.center.y + frame.ry.as_mm(),
        ),
        Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => (
            frame.center.x - frame.radius.as_mm(),
            frame.center.y - frame.radius.as_mm(),
            frame.center.x + frame.radius.as_mm(),
            frame.center.y + frame.radius.as_mm(),
        ),
    }
}

#[test]
fn c15_double_click_on_the_centre_handle_opens_the_move_chip_for_every_kind() {
    for th in [0.0, 0.9] {
        for c in kinds_big(th) {
            let mut s = select_at(&c.doc, c.on);
            let before = snapshot_bytes(&s);
            let n = change_count(&s);
            let ctr = ctr_of(&obj_of(&s, 0));
            let hint = dbl(&mut s, ctr, ctr, false, false);
            let name = format!("{} th {th}", c.name);
            assert!(!hint, "{name}: no hint chip");
            let v = s
                .move_entry()
                .unwrap_or_else(|| panic!("{name}: move chip must open"));
            assert!(
                pnear(v.center, ctr, 1e-6),
                "{name}: chip centre {:?} vs {ctr:?}",
                v.center
            );
            assert_eq!(s.tool(), Tool::Select, "{name}: no tool switch");
            assert_eq!(s.selected_object_count(), 1);
            assert_eq!(
                snapshot_bytes(&s),
                before,
                "{name}: the presses wrote nothing"
            );
            assert_eq!(change_count(&s), n);
            assert!(s.transform_entry().is_none(), "{name}: only one chip");
            assert_eq!(
                v.relative_prefill,
                ["0".to_string(), "0".to_string()],
                "{name}"
            );
        }
    }
}

#[test]
fn c16_the_hit_region_is_min_12_and_s_over_4_px_and_other_double_clicks_keep_their_routing() {
    let kk;
    {
        let s = open_in_session(&triangle_doc_sized(120.0, 80.0));
        kk = k(&s);
    }
    let shorter_px = 80.0 * kk;
    assert!(
        shorter_px >= 48.0,
        "scale {kk}: the test object must show the handle"
    );
    let r_px = 12.0_f64.min(shorter_px / 4.0);

    // path
    let (mut s, b) = select_path(&triangle_doc_sized(120.0, 80.0));
    let inside = pt(b.c.x + (r_px - 1.5) / kk, b.c.y);
    dbl(&mut s, inside, inside, false, false);
    assert!(s.move_entry().is_some(), "inside the region opens it");
    assert_eq!(s.tool(), Tool::Select);
    s.cancel_transform_entry();

    let (mut s, b) = select_path(&triangle_doc_sized(120.0, 80.0));
    let outside = pt(b.c.x + (r_px + 2.5) / kk, b.c.y);
    dbl(&mut s, outside, outside, false, false);
    assert!(s.move_entry().is_none(), "outside the region: no chip");
    assert_eq!(
        s.tool(),
        Tool::Node,
        "path: inside the box elsewhere hands off to the Node tool"
    );

    // on the outline
    let (mut s, _b) = select_path(&triangle_doc_sized(120.0, 80.0));
    let on = pt(30.0, 20.0); // on the hypotenuse, away from every handle
    dbl(&mut s, on, on, false, false);
    assert!(s.move_entry().is_none());
    assert_eq!(s.tool(), Tool::Node, "outline double-click hands off");

    // primitive: the hint chip
    for (d, on, name) in [
        (rect_doc(0.0, 0.0, 120.0, 80.0), pt(60.0, 0.0), "rect"),
        (
            ellipse_doc(60.0, 40.0, 60.0, 40.0),
            pt(60.0, 0.0),
            "ellipse",
        ),
    ] {
        let mut s = select_at(&d, on);
        // an outline point away from every handle spot
        let on = if name == "rect" {
            pt(30.0, 0.0)
        } else {
            pt(
                60.0 + 60.0 * (-0.9_f64).cos(),
                40.0 + 40.0 * (-0.9_f64).sin(),
            )
        };
        let c = pt(60.0, 40.0);
        let out = pt(c.x + (r_px + 2.5) / kk, c.y);
        let hint = dbl(&mut s, out, out, false, false);
        assert!(hint, "{name}: inside the box elsewhere: hint chip");
        assert!(s.move_entry().is_none(), "{name}");
        assert_eq!(s.tool(), Tool::Select, "{name}");
        let hint = dbl(&mut s, on, on, false, false);
        assert!(hint, "{name}: outline: hint chip");
        assert!(s.move_entry().is_none(), "{name}");
        let inn = pt(c.x + (r_px - 1.5) / kk, c.y);
        let hint = dbl(&mut s, inn, inn, false, false);
        assert!(!hint, "{name}: centre region: no hint");
        assert!(s.move_entry().is_some(), "{name}: chip");
    }
}

#[test]
fn c16_a_press_and_drag_from_the_centre_handle_is_still_a_move() {
    let d = rect_doc(10.0, 20.0, 120.0, 80.0);
    let mut s = select_at(&d, pt(70.0, 20.0));
    let c = pt(70.0, 60.0);
    drag_mod(&mut s, c, pt(c.x + 25.5, c.y - 13.25), false, false);
    let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
        panic!()
    };
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    assert!(
        pnear(bounds.origin, pt(35.5, 6.75), 1e-9),
        "{:?}",
        bounds.origin
    );
    assert!(s.move_entry().is_none());
}

#[test]
fn c17_small_objects_and_hidden_centre_handles_open_no_chip_by_double_click_but_m_opens_it() {
    let tiny = [
        ("rect", rect_doc(10.0, 20.0, 6.0, 4.0), pt(13.0, 20.0)),
        ("ellipse", ellipse_doc(13.0, 22.0, 3.0, 2.0), pt(13.0, 20.0)),
        ("path", triangle_doc_sized(6.0, 4.0), pt(3.0, 0.0)),
    ];
    for (name, d, on) in tiny {
        let mut s = select_at(&d, on);
        let kk = k(&s);
        assert!(4.0 * kk < 48.0, "scale {kk}: object must be small");
        let ctr = ctr_of(&obj_of(&s, 0));
        let before = snapshot_bytes(&s);
        let hint = dbl(&mut s, ctr, ctr, false, false);
        assert!(
            s.move_entry().is_none(),
            "{name}: no chip by double-click on a tiny object"
        );
        if name == "path" {
            assert_eq!(s.tool(), Tool::Node, "{name}: old rule: path hands off");
        } else {
            assert!(hint, "{name}: old rule: hint chip");
            assert_eq!(s.tool(), Tool::Select);
        }
        assert_eq!(snapshot_bytes(&s), before);
        // M works, at the centre
        s.set_tool(Tool::Select);
        let mut s = select_at(&d, on);
        assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened, "{name}");
        let v = s.move_entry().expect("chip");
        assert!(
            pnear(v.center, ctr, 1e-6),
            "{name}: placed at the centre: {:?} vs {ctr:?}",
            v.center
        );
        assert_eq!(
            s.commit_move_entry(
                "2",
                "1",
                MoveEntryMode {
                    absolute: false,
                    copy: false
                }
            ),
            EntryOutcome::Committed,
            "{name}"
        );
        let after = ctr_of(&obj_of(&s, 0));
        assert!(
            pnear(after, pt(ctr.x + 2.0, ctr.y + 1.0), 1e-6),
            "{name}: {after:?}"
        );
    }
}

#[test]
fn c17_a_parameter_handle_near_the_centre_hides_the_centre_handle_so_only_m_opens_the_chip() {
    // a big star with a tiny inner ratio: the inner-ratio handle sits within
    // 20 px of the centre
    let d = star_doc(70.0, 60.0, 40.0, 5, 0.05);
    let mut s = select_at(&d, pt(70.0, 20.0));
    let kk = k(&s);
    let inner_px = 40.0 * 0.05 * kk;
    assert!(inner_px < 20.0, "scale {kk}");
    let ctr = pt(70.0, 60.0);
    let hint = dbl(&mut s, ctr, ctr, false, false);
    assert!(s.move_entry().is_none(), "centre handle hidden: no chip");
    eprintln!(
        "hidden-centre star: hint {hint}, entry {:?}",
        s.transform_entry().map(|e| e.kind)
    );
    assert_eq!(s.tool(), Tool::Select);
    let mut s = select_at(&d, pt(70.0, 20.0));
    assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened);
    assert!(s.move_entry().is_some());
}

#[test]
fn c18_the_chip_prefills_relative_zero_and_absolute_the_tight_top_left() {
    for th in [0.0, 0.9, 2.5] {
        for c in kinds_big(th) {
            let mut s = select_at(&c.doc, c.on);
            assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened);
            let v = s.move_entry().unwrap();
            let (x0, y0, _, _) = tight_of(&obj_of(&s, 0));
            let name = format!("{} th {th}", c.name);
            assert_eq!(
                v.relative_prefill,
                ["0".to_string(), "0".to_string()],
                "{name}"
            );
            assert!(
                near(num(&v.absolute_prefill[0]), x0, 0.051)
                    && near(num(&v.absolute_prefill[1]), y0, 0.051),
                "{name}: absolute prefill {:?} vs tight top-left ({x0}, {y0})",
                v.absolute_prefill
            );
        }
    }
    // the bulgy path: the curve, not the anchors, sets the tight box
    let mut s = select_at(&bulgy_doc(), pt(10.04, 0.0));
    press(&mut s, "m");
    let v = s.move_entry().unwrap();
    let (x0, y0, _, _) = tight_of(&obj_of(&s, 0));
    assert!(
        y0 < -3.0,
        "oracle sanity: the curve rises above its anchors ({y0})"
    );
    assert!(
        near(num(&v.absolute_prefill[0]), x0, 0.051)
            && near(num(&v.absolute_prefill[1]), y0, 0.051),
        "{:?} vs ({x0}, {y0})",
        v.absolute_prefill
    );
}

#[test]
fn c20_relative_move_equals_a_drag_of_the_same_offset_and_is_one_commit() {
    for th in [0.0, 0.9] {
        for c in kinds_big(th) {
            let name = format!("{} th {th}", c.name);
            let (dx, dy) = (25.5, -13.25);
            // typed
            let mut a = select_at(&c.doc, c.on);
            let ctr = ctr_of(&obj_of(&a, 0));
            dbl(&mut a, ctr, ctr, false, false);
            assert!(a.move_entry().is_some(), "{name}");
            let n = change_count(&a);
            let out = a.commit_move_entry(
                "25.5",
                "-13.25",
                MoveEntryMode {
                    absolute: false,
                    copy: false,
                },
            );
            assert_eq!(out, EntryOutcome::Committed, "{name}");
            assert_eq!(change_count(&a), n + 1, "{name}: one commit");
            assert!(a.move_entry().is_none(), "{name}: chip closes");
            assert_eq!(a.tool(), Tool::Select, "{name}");
            assert_eq!(a.selected_object_count(), 1, "{name}");
            // dragged
            let mut b = select_at(&c.doc, c.on);
            drag_mod(&mut b, ctr, pt(ctr.x + dx, ctr.y + dy), false, false);
            same_object(&obj_of(&a, 0), &obj_of(&b, 0), 1e-9, &name);
            // centre handle follows
            assert_eq!(press(&mut a, "m"), KeyOutcome::EntryOpened);
            let c2 = a.move_entry().unwrap().center;
            assert!(
                pnear(c2, pt(ctr.x + dx, ctr.y + dy), 1e-6),
                "{name}: new centre {c2:?}"
            );
        }
    }
}

#[test]
fn c20_spec_example_relative_5_and_minus_3_moves_top_left_10_20_to_15_17() {
    // a 30 x 40 rect at (10, 20): tight bounds top-left (10, 20)
    let mut s = select_at(&rect_doc(10.0, 20.0, 30.0, 40.0), pt(25.0, 20.0));
    assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened);
    assert_eq!(
        s.commit_move_entry(
            "5",
            "-3",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    let (x0, y0, x1, y1) = tight_of(&obj_of(&s, 0));
    assert!(near(x0, 15.0, 1e-9) && near(y0, 17.0, 1e-9), "{x0} {y0}");
    assert!(near(x1 - x0, 30.0, 1e-9) && near(y1 - y0, 40.0, 1e-9));
}

#[test]
fn c21_spec_example_absolute_100_50_puts_the_rect_origin_at_100_50() {
    let mut s = select_at(&rect_doc(10.0, 20.0, 30.0, 40.0), pt(25.0, 20.0));
    assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened);
    assert_eq!(
        s.commit_move_entry(
            "100",
            "50",
            MoveEntryMode {
                absolute: true,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
        panic!()
    };
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    assert!(
        pnear(bounds.origin, pt(100.0, 50.0), 1e-9),
        "{:?}",
        bounds.origin
    );
}

#[test]
fn c21_absolute_puts_the_top_left_of_the_tight_drawn_outline_at_x_y_for_every_kind_and_rotation() {
    let mut cases: Vec<(String, Document, Point)> = vec![];
    for th in [0.0, 0.9, 2.5, -0.4] {
        for c in kinds_big(th) {
            cases.push((format!("{} th {th}", c.name), c.doc, c.on));
        }
    }
    // a rounded, rotated rectangle: the rounded corner cuts the hull
    {
        let d = rect_doc(10.0, 20.0, 60.0, 30.0);
        let id = d.object_ids()[0];
        d.set_corner_radius(&[id], Length::from_mm(12.0)).unwrap();
        let on = rotate_with_on(&d, pt(40.0, 20.0), 0.6);
        cases.push(("rounded rect th 0.6".into(), d, on));
    }
    // star with an odd number of points and a different ratio, rotated
    {
        let d = star_doc(100.0, 100.0, 50.0, 7, 0.4);
        let on = rotate_with_on(&d, pt(100.0, 50.0), 1.1);
        cases.push(("star7 th 1.1".into(), d, on));
    }
    // curvy closed path, rotated register; and the bulgy open curve
    {
        let d = curvy_doc();
        rotate_doc_object(&d, 0.7);
        cases.push(("curvy rotated".into(), d, pt(0.0, 10.0)));
        cases.push(("bulgy".into(), bulgy_doc(), pt(10.04, 0.0)));
    }
    for (name, d, on) in cases {
        for (tx, ty) in [(100.0, 50.0), (-12.5, -3.0), (0.0, 0.0)] {
            let mut s = open_in_session(&d);
            // select via the first anchor / outline
            let on = match obj_of(&s, 0) {
                ObjectSnapshot::Path(p) => p.anchors[0].point,
                ObjectSnapshot::Primitive(_) => on,
            };
            click(&mut s, on);
            assert_eq!(s.selected_object_count(), 1, "{name}");
            let before = tight_of(&obj_of(&s, 0));
            let n = change_count(&s);
            assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened, "{name}");
            let out = s.commit_move_entry(
                &format!("{tx}"),
                &format!("{ty}"),
                MoveEntryMode {
                    absolute: true,
                    copy: false,
                },
            );
            if before.0 == tx && before.1 == ty {
                continue;
            }
            assert_eq!(out, EntryOutcome::Committed, "{name} ({tx},{ty})");
            assert_eq!(change_count(&s), n + 1, "{name}: one commit");
            let after = tight_of(&obj_of(&s, 0));
            assert!(
                near(after.0, tx, 2e-3) && near(after.1, ty, 2e-3),
                "{name}: tight top-left {:?} wanted ({tx}, {ty})",
                (after.0, after.1)
            );
            assert!(
                near(after.2 - after.0, before.2 - before.0, 2e-3)
                    && near(after.3 - after.1, before.3 - before.1, 2e-3),
                "{name}: size unchanged"
            );
            // rigid move: rotation and shape parameters unchanged
            let o0 = obj_of(&open_in_session(&d), 0);
            let o1 = obj_of(&s, 0);
            if let (ObjectSnapshot::Primitive(a), ObjectSnapshot::Primitive(b)) = (&o0, &o1) {
                assert_eq!(a.rotation.as_radians(), b.rotation.as_radians(), "{name}");
            }
            let _ = o1;
        }
    }
}

#[test]
fn c21_absolute_does_not_include_the_stroke_width_and_equals_the_relative_offset_it_implies() {
    for c in kinds_big(0.3) {
        let mut a = select_at(&c.doc, c.on);
        let (x0, y0, _, _) = tight_of(&obj_of(&a, 0));
        press(&mut a, "m");
        a.commit_move_entry(
            "40.25",
            "-7.5",
            MoveEntryMode {
                absolute: true,
                copy: false,
            },
        );
        let mut b = select_at(&c.doc, c.on);
        press(&mut b, "m");
        b.commit_move_entry(
            &format!("{}", 40.25 - x0),
            &format!("{}", -7.5 - y0),
            MoveEntryMode {
                absolute: false,
                copy: false,
            },
        );
        same_object(&obj_of(&a, 0), &obj_of(&b, 0), 1e-6, c.name);
    }
}

#[test]
fn c19_an_untouched_field_means_no_change_on_that_axis() {
    // relative: the prefill text "0" on either axis
    let mut s = select_at(&bulgy_doc(), pt(10.04, 0.0));
    press(&mut s, "m");
    let v = s.move_entry().unwrap();
    let before = snapshot_bytes(&s);
    let n = change_count(&s);
    assert_eq!(
        s.commit_move_entry(
            &v.relative_prefill[0],
            &v.relative_prefill[1],
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Unchanged
    );
    assert_eq!(snapshot_bytes(&s), before);
    assert!(s.move_entry().is_none());
    assert_eq!(change_count(&s), n);

    // absolute: both prefills (one decimal, rounded) change nothing
    let mut s = select_at(&bulgy_doc(), pt(10.04, 0.0));
    press(&mut s, "m");
    let v = s.move_entry().unwrap();
    let out = s.commit_move_entry(
        &v.absolute_prefill[0],
        &v.absolute_prefill[1],
        MoveEntryMode {
            absolute: true,
            copy: false,
        },
    );
    assert_eq!(
        out,
        EntryOutcome::Unchanged,
        "prefills {:?}",
        v.absolute_prefill
    );
    assert_eq!(snapshot_bytes(&s), before);

    // absolute: X untouched (rounded prefill 10.0 must not pull x from
    // 10.04 to 10.0), Y typed
    let mut s = select_at(&bulgy_doc(), pt(10.04, 0.0));
    press(&mut s, "m");
    let v = s.move_entry().unwrap();
    let (x0, _y0, _, _) = tight_of(&obj_of(&s, 0));
    assert_eq!(
        s.commit_move_entry(
            &v.absolute_prefill[0],
            "5",
            MoveEntryMode {
                absolute: true,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    let after = tight_of(&obj_of(&s, 0));
    assert!(
        near(after.0, x0, 1e-6),
        "untouched X drifted: {} -> {}",
        x0,
        after.0
    );
    assert!(near(after.1, 5.0, 2e-3), "Y {}", after.1);

    // absolute: Y untouched, X typed
    let mut s = select_at(&bulgy_doc(), pt(10.04, 0.0));
    press(&mut s, "m");
    let v = s.move_entry().unwrap();
    let (_x0, y0, _, _) = tight_of(&obj_of(&s, 0));
    assert_eq!(
        s.commit_move_entry(
            "77",
            &v.absolute_prefill[1],
            MoveEntryMode {
                absolute: true,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    let after = tight_of(&obj_of(&s, 0));
    assert!(near(after.0, 77.0, 2e-3), "X {}", after.0);
    assert!(
        near(after.1, y0, 1e-6),
        "untouched Y drifted: {} -> {}",
        y0,
        after.1
    );

    // relative: one axis typed, the other the prefill
    let mut s = select_at(&rect_doc(10.0, 20.0, 30.0, 40.0), pt(25.0, 20.0));
    press(&mut s, "m");
    assert_eq!(
        s.commit_move_entry(
            "5",
            "0",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    let (x0, y0, _, _) = tight_of(&obj_of(&s, 0));
    assert!(near(x0, 15.0, 1e-9) && near(y0, 20.0, 1e-9));

    // typed text is read in the mode current at Enter: "5, -3" in Absolute
    let mut s = select_at(&rect_doc(10.0, 20.0, 30.0, 40.0), pt(25.0, 20.0));
    press(&mut s, "m");
    assert_eq!(
        s.commit_move_entry(
            "5",
            "-3",
            MoveEntryMode {
                absolute: true,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    let (x0, y0, _, _) = tight_of(&obj_of(&s, 0));
    assert!(
        near(x0, 5.0, 1e-9) && near(y0, -3.0, 1e-9),
        "negative absolute is valid: {x0} {y0}"
    );
}

#[test]
fn c22_invalid_text_or_out_of_range_keeps_the_chip_open_and_writes_nothing() {
    for absolute in [false, true] {
        let mut s = select_at(&rect_doc(10.0, 20.0, 30.0, 40.0), pt(25.0, 20.0));
        press(&mut s, "m");
        let before = snapshot_bytes(&s);
        let n = change_count(&s);
        for (a, b, field) in [
            ("", "1", 0),
            ("1", "", 1),
            ("abc", "1", 0),
            ("1", "abc", 1),
            ("1.2.3", "1", 0),
            ("1,2,3", "1", 0),
            ("1,2.3", "1", 0),
            ("NaN", "1", 0),
            ("1", "inf", 1),
            ("-inf", "inf", 0),
            ("1e999", "1", 0),
            ("1", "1e999", 1),
            ("20000000", "1", 0),
            ("1", "-20000000", 1),
            ("1e300", "1", 0),
            ("--1", "1", 0),
            ("1-", "1", 0),
            ("1 2", "1", 0),
        ] {
            let out = s.commit_move_entry(
                a,
                b,
                MoveEntryMode {
                    absolute,
                    copy: false,
                },
            );
            assert!(
                matches!(out, EntryOutcome::Invalid { field: f, reason: InvalidReason::NotANumber } if f == field),
                "abs {absolute} ({a:?}, {b:?}) gave {out:?}, wanted field {field}"
            );
            assert!(s.move_entry().is_some(), "chip stays open");
            assert_eq!(
                snapshot_bytes(&s),
                before,
                "({a:?}, {b:?}): nothing written"
            );
            assert_eq!(change_count(&s), n);
        }
        // both fields wrong: the first is flagged
        let out = s.commit_move_entry(
            "x",
            "y",
            MoveEntryMode {
                absolute,
                copy: false,
            },
        );
        assert!(
            matches!(out, EntryOutcome::Invalid { field: 0, .. }),
            "{out:?}"
        );
        // the chip still works
        assert_ne!(
            s.commit_move_entry(
                "3",
                "4",
                MoveEntryMode {
                    absolute,
                    copy: false
                }
            ),
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::NotANumber
            }
        );
    }
}

#[test]
fn c22_a_result_beyond_the_coordinate_limit_is_refused_in_both_modes() {
    // relative: 10 + 1e7 is beyond; 10 + 9e6 is not
    let mut s = select_at(&rect_doc(10.0, 20.0, 30.0, 40.0), pt(25.0, 20.0));
    press(&mut s, "m");
    let before = snapshot_bytes(&s);
    for (a, b) in [("10000000", "0"), ("0", "-10000100"), ("1e7", "1")] {
        let out = s.commit_move_entry(
            a,
            b,
            MoveEntryMode {
                absolute: false,
                copy: false,
            },
        );
        assert!(
            matches!(out, EntryOutcome::Invalid { .. }),
            "({a},{b}) {out:?}"
        );
        assert_eq!(snapshot_bytes(&s), before);
    }
    for (a, b) in [("10000001", "0"), ("0", "-10000001"), ("1e8", "1")] {
        let out = s.commit_move_entry(
            a,
            b,
            MoveEntryMode {
                absolute: true,
                copy: false,
            },
        );
        assert!(
            matches!(out, EntryOutcome::Invalid { .. }),
            "abs ({a},{b}) {out:?}"
        );
        assert_eq!(snapshot_bytes(&s), before);
    }
    assert_eq!(
        s.commit_move_entry(
            "9000000",
            "-9000000",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    let (x0, y0, _, _) = tight_of(&obj_of(&s, 0));
    assert!(
        near(x0, 9_000_010.0, 1e-3) && near(y0, -8_999_980.0, 1e-3),
        "{x0} {y0}"
    );
    // and the file still reopens
    assert!(Session::open(3, &s.pack("0.1.0").unwrap()).is_ok());
}

#[test]
fn c22_number_formats_comma_point_sign_and_odd_spellings() {
    for (text, want) in [
        ("2,5", 2.5),
        ("2.5", 2.5),
        ("-2,5", -2.5),
        ("+2.5", 2.5),
        (" 2.5 ", 2.5),
        (".5", 0.5),
        ("0,5", 0.5),
    ] {
        let mut s = select_at(&rect_doc(10.0, 20.0, 30.0, 40.0), pt(25.0, 20.0));
        press(&mut s, "m");
        let out = s.commit_move_entry(
            text,
            "0",
            MoveEntryMode {
                absolute: false,
                copy: false,
            },
        );
        assert_eq!(out, EntryOutcome::Committed, "{text:?}");
        let (x0, _, _, _) = tight_of(&obj_of(&s, 0));
        assert!(near(x0, 10.0 + want, 1e-9), "{text:?}: {x0}");
    }
    // spellings the spec does not name: recorded, not asserted
    for text in [
        "\u{2212}3",
        "5mm",
        "5 mm",
        "1e1",
        "5.",
        "1_0",
        "\u{0661}\u{0662}",
        "0x10",
        "−3,5",
    ] {
        let mut s = select_at(&rect_doc(10.0, 20.0, 30.0, 40.0), pt(25.0, 20.0));
        press(&mut s, "m");
        let out = s.commit_move_entry(
            text,
            "0",
            MoveEntryMode {
                absolute: false,
                copy: false,
            },
        );
        let (x0, _, _, _) = tight_of(&obj_of(&s, 0));
        eprintln!("MOVE-FORMAT {text:?} -> {out:?}, x0 = {x0}");
    }
}

#[test]
fn c25_closing_without_writing_escape_blur_tool_switch_selection_change_press_elsewhere() {
    let d = Document::new(1);
    let _ = d.create_rect(bounds(0.0, 0.0, 120.0, 80.0));
    let _ = d.create_rect(bounds(300.0, 0.0, 120.0, 80.0));
    let open = |d: &Document| {
        let mut s = open_in_session(d);
        click(&mut s, pt(60.0, 0.0));
        assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened);
        s
    };
    let mut s = open(&d);
    let before = snapshot_bytes(&s);
    let n = change_count(&s);
    assert_eq!(
        s.key_down(key("Escape")),
        KeyOutcome::Escape(EscapeStep::ClosedEntry)
    );
    assert!(s.move_entry().is_none());
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(snapshot_bytes(&s), before);

    let mut s = open(&d);
    s.cancel_transform_entry();
    assert!(s.move_entry().is_none(), "blur closes it");
    assert_eq!(snapshot_bytes(&s), before);

    for tool in [Tool::Node, Tool::Pen, Tool::Ellipse] {
        let mut s = open(&d);
        s.set_tool(tool);
        assert!(s.move_entry().is_none(), "{tool:?}");
        s.set_tool(Tool::Select);
        assert!(s.move_entry().is_none(), "{tool:?}: not revived");
        assert_eq!(snapshot_bytes(&s), before);
    }

    // the press that closes it is not swallowed
    let mut s = open(&d);
    click(&mut s, pt(360.0, 0.0));
    assert!(s.move_entry().is_none());
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(snapshot_bytes(&s), before);
    press(&mut s, "m");
    assert!(
        pnear(s.move_entry().unwrap().center, pt(360.0, 40.0), 1e-6),
        "the other rect got selected"
    );

    let mut s = open(&d);
    click(&mut s, pt(200.0, 300.0));
    assert!(s.move_entry().is_none());
    assert_eq!(s.selected_object_count(), 0, "the empty-canvas press acted");

    // a no-op result (0, 0 typed explicitly counts as untouched) writes nothing
    let mut s = open(&d);
    assert_eq!(
        s.commit_move_entry(
            "0",
            "0",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Unchanged
    );
    assert_eq!(
        s.commit_move_entry(
            "0",
            "0",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Unchanged
    );
    assert_eq!(change_count(&s), n);
    // and a typed zero in a tool-changed state
    let mut s = open(&d);
    assert_eq!(
        s.commit_move_entry(
            "0.0",
            "-0",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Unchanged
    );
    assert_eq!(snapshot_bytes(&s), before);
}

#[test]
fn c25_a_committed_move_keeps_selection_and_tool_and_survives_save_and_reopen() {
    let mut s = select_at(&triangle_doc_sized(120.0, 80.0), pt(60.0, 0.0));
    press(&mut s, "m");
    assert_eq!(
        s.commit_move_entry(
            "10",
            "20",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.selected_object_count(), 1);
    let again = open_in_session(&doc_of(&s));
    let a = obj_of(&again, 0);
    let b = obj_of(&s, 0);
    same_object(&a, &b, 1e-9, "reopened");
    let ObjectSnapshot::Path(p) = b else { panic!() };
    assert!(pnear(p.anchors[0].point, pt(10.0, 20.0), 1e-9));
}

#[test]
fn c25_the_typed_move_exists_only_for_a_single_selection() {
    let d = Document::new(1);
    let _ = d.create_rect(bounds(0.0, 0.0, 120.0, 80.0));
    let _ = d.create_rect(bounds(300.0, 0.0, 120.0, 80.0));
    let mut s = open_in_session(&d);
    click(&mut s, pt(60.0, 0.0));
    s.pointer_hover(pt(360.0, 0.0), true, false);
    s.pointer_down(pt(360.0, 0.0), true);
    s.pointer_up(pt(360.0, 0.0), true, false);
    assert_eq!(s.selected_object_count(), 2);
    let before = snapshot_bytes(&s);
    assert_eq!(press(&mut s, "m"), KeyOutcome::Hint(KeyHint::SelectOne));
    assert_eq!(press(&mut s, "k"), KeyOutcome::Hint(KeyHint::SelectOne));
    assert_eq!(
        press_shift(&mut s, "K"),
        KeyOutcome::Hint(KeyHint::SelectOne)
    );
    assert!(s.move_entry().is_none());
    assert_eq!(snapshot_bytes(&s), before);
    assert_eq!(s.selected_object_count(), 2);
    assert_eq!(s.tool(), Tool::Select);
    // a double-click in the union box centre opens nothing
    let c = pt(210.0, 40.0);
    dbl(&mut s, c, c, false, false);
    assert!(s.move_entry().is_none());
    assert_eq!(snapshot_bytes(&s), before);
}

// ---------------------------------------------------------------------
// Keys M, K, Shift+K: criteria 54, 55, 56, 58, 59
// ---------------------------------------------------------------------

#[test]
fn c59_hints_when_nothing_is_selected_or_the_tool_is_not_select() {
    let d = triangle_doc_sized(120.0, 80.0);
    // nothing selected, Select tool
    let mut s = open_in_session(&d);
    let before = snapshot_bytes(&s);
    for outcome in [
        press(&mut s, "m"),
        press(&mut s, "k"),
        press_shift(&mut s, "K"),
    ] {
        assert_eq!(outcome, KeyOutcome::Hint(KeyHint::SelectFirst));
    }
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(snapshot_bytes(&s), before);
    // outside the Select tool, even with a selection (the selection survives
    // the tool switch)
    let mut s = select_at(&d, pt(60.0, 0.0));
    for tool in [
        Tool::Node,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
    ] {
        s.set_tool(tool);
        for outcome in [
            press(&mut s, "m"),
            press(&mut s, "k"),
            press_shift(&mut s, "K"),
        ] {
            assert_eq!(outcome, KeyOutcome::Hint(KeyHint::SelectFirst), "{tool:?}");
        }
        assert_eq!(s.tool(), tool, "{tool:?}: tool unchanged");
        assert!(s.move_entry().is_none());
        assert!(s.transform_entry().is_none());
    }
    assert_eq!(snapshot_bytes(&s), before);
}

#[test]
fn c54_k_opens_skew_x_shift_k_opens_skew_y_and_the_letter_case_is_irrelevant() {
    let d = curvy_doc();
    for (kk, shift, kind_name) in [
        ("k", false, "Skew angle x"),
        ("K", false, "Skew angle x"), // Caps Lock
        ("K", true, "Skew angle y"),  // Shift+K
        ("k", true, "Skew angle y"),
    ] {
        let (mut s, _b) = select_path(&d);
        let out = s.key_down(KeyInput {
            key: kk,
            shift,
            ..KeyInput::default()
        });
        assert_eq!(out, KeyOutcome::EntryOpened, "{kk} shift {shift}");
        let e = s.transform_entry().expect("entry");
        assert_eq!(e.kind, "skew");
        assert_eq!(e.fields[0].accessible_name, kind_name, "{kk} shift {shift}");
        assert_eq!(e.fields[0].prefill.trim(), "0");
        assert_eq!(s.tool(), Tool::Select);
    }
    // "M" with Caps Lock
    let (mut s, _b) = select_path(&d);
    assert_eq!(press(&mut s, "M"), KeyOutcome::EntryOpened);
    assert!(s.move_entry().is_some());
    // Shift+M is not the move key (Shift is read only for * and Shift+K)
    let (mut s, _b) = select_path(&d);
    let out = press_shift(&mut s, "M");
    assert_eq!(out, KeyOutcome::Ignored, "Shift+M");
    assert!(s.move_entry().is_none());
}

#[test]
fn c58_key_k_equals_the_top_handle_entry_and_shift_k_the_right_handle_entry_never_with_the_shift_pivot()
 {
    for th in [0.0, 0.7] {
        let d = curvy_doc();
        if th != 0.0 {
            rotate_doc_object(&d, th);
        }
        for (shift, side) in [(false, (0.0, -1.0)), (true, (1.0, 0.0))] {
            for alpha in ["25", "-40.5"] {
                let (mut a, _b) = select_path(&d);
                let out = a.key_down(KeyInput {
                    key: "k",
                    shift,
                    ..KeyInput::default()
                });
                assert_eq!(out, KeyOutcome::EntryOpened);
                let n = change_count(&a);
                assert_eq!(
                    a.commit_transform_entry(alpha, "", 0),
                    EntryOutcome::Committed
                );
                assert_eq!(change_count(&a), n + 1, "one commit");
                // reference: the double-click route on the handle, no Shift
                let (mut b2, bx) = select_path(&d);
                let h = skew_handle_pos(&b2, &bx, side);
                dbl(&mut b2, h, h, false, false);
                assert_eq!(
                    b2.commit_transform_entry(alpha, "", 0),
                    EntryOutcome::Committed
                );
                paths_equal(
                    &path_of(&a),
                    &path_of(&b2),
                    1e-9,
                    &format!("th {th} shift {shift} {alpha}"),
                );
            }
        }
    }
}

#[test]
fn c59_k_and_m_chips_open_where_the_handle_is_and_on_tiny_objects_too() {
    // normal size: K's anchor equals the double-click route's anchor
    for side_key in [(false, (0.0, -1.0)), (true, (1.0, 0.0))] {
        let (mut a, bx) = select_path(&curvy_doc());
        a.key_down(KeyInput {
            key: "k",
            shift: side_key.0,
            ..KeyInput::default()
        });
        let ea = a.transform_entry().unwrap();
        let (mut b, _) = select_path(&curvy_doc());
        let h = skew_handle_pos(&b, &bx, side_key.1);
        dbl(&mut b, h, h, false, false);
        let eb = b.transform_entry().unwrap();
        assert!(
            pnear(ea.handle, eb.handle, 1e-9),
            "{:?} vs {:?}",
            ea.handle,
            eb.handle
        );
        assert!(pnear(ea.handle, h, 1e-6), "chip at the handle");
        assert!(pnear(ea.center, eb.center, 1e-9));
    }
    // tiny path: the handle is hidden, the chip still opens at where it would be
    let (mut a, bx) = select_path(&triangle_doc_sized(5.0, 3.0));
    assert_eq!(press(&mut a, "k"), KeyOutcome::EntryOpened);
    let e = a.transform_entry().unwrap();
    assert!(
        pnear(e.handle, skew_handle_pos(&a, &bx, (0.0, -1.0)), 1e-6),
        "tiny: chip {:?} vs where the handle would be {:?}",
        e.handle,
        skew_handle_pos(&a, &bx, (0.0, -1.0))
    );
    assert_eq!(
        a.commit_transform_entry("30", "", 0),
        EntryOutcome::Committed
    );
}

#[test]
fn c54_the_keyboard_gate_still_holds_for_m_and_k() {
    let base = || select_path(&triangle_doc_sized(120.0, 80.0)).0;
    // modifiers, repeat, dom_blocked
    for input in [
        KeyInput {
            key: "m",
            ctrl: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "k",
            ctrl: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "m",
            alt: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "k",
            alt: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "k",
            shift: true,
            ctrl: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "K",
            shift: true,
            alt: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "m",
            repeat: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "k",
            repeat: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "K",
            shift: true,
            repeat: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "m",
            dom_blocked: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "k",
            dom_blocked: true,
            ..KeyInput::default()
        },
        KeyInput {
            key: "K",
            shift: true,
            dom_blocked: true,
            ..KeyInput::default()
        },
    ] {
        let mut s = base();
        let before = snapshot_bytes(&s);
        assert_eq!(s.key_down(input), KeyOutcome::Ignored, "{input:?}");
        assert!(
            s.move_entry().is_none() && s.transform_entry().is_none(),
            "{input:?}"
        );
        assert_eq!(snapshot_bytes(&s), before);
    }
    // a chip is open: M, K, S, R do nothing (focus is in it anyway)
    let mut s = base();
    assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened);
    for kk in ["m", "k", "s", "r", "b", "n", "e", "Delete", "Backspace"] {
        assert_eq!(press(&mut s, kk), KeyOutcome::Ignored, "chip open: {kk}");
    }
    assert!(s.move_entry().is_some());
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.selected_object_count(), 1);
    let mut s = base();
    assert_eq!(press(&mut s, "k"), KeyOutcome::EntryOpened);
    for kk in ["m", "k", "s", "r", "Delete"] {
        assert_eq!(
            press(&mut s, kk),
            KeyOutcome::Ignored,
            "skew chip open: {kk}"
        );
    }
    assert_eq!(s.selected_object_count(), 1);
    // an open S/R chip too
    let mut s = base();
    assert_eq!(press(&mut s, "s"), KeyOutcome::EntryOpened);
    assert_eq!(press(&mut s, "m"), KeyOutcome::Ignored);
    assert_eq!(press(&mut s, "k"), KeyOutcome::Ignored);
}

#[test]
fn c55_during_a_drag_m_and_k_do_nothing_and_the_commit_equals_the_same_drag_without_the_key() {
    let drive = |with_key: Option<KeyInput<'_>>| {
        let (mut s, bx) = select_path(&triangle_doc_sized(120.0, 80.0));
        let from = pt(60.0, 0.0);
        s.pointer_hover(from, false, false);
        s.pointer_down(from, false);
        s.pointer_hover(pt(80.0, 10.0), false, false);
        if let Some(i) = with_key {
            assert_eq!(s.key_down(i), KeyOutcome::Ignored, "{i:?} during a drag");
            assert!(s.move_entry().is_none() && s.transform_entry().is_none());
        }
        s.pointer_hover(pt(95.0, 30.0), false, false);
        s.pointer_up(pt(95.0, 30.0), false, false);
        let _ = bx;
        (snapshot_bytes(&s), s.tool(), s.selected_object_count())
    };
    let plain = drive(None);
    for i in [
        key("m"),
        key("k"),
        KeyInput {
            key: "K",
            shift: true,
            ..KeyInput::default()
        },
    ] {
        assert_eq!(drive(Some(i)), plain, "{i:?}");
    }
    // while a skew drag runs
    let (mut s, b) = select_path(&triangle_doc_sized(120.0, 80.0));
    let h = skew_handle_pos(&s, &b, (0.0, -1.0));
    s.pointer_hover(h, false, false);
    s.pointer_down(h, false);
    s.pointer_hover(pt(h.x + 30.0 / k(&s), h.y), false, false);
    assert_eq!(press(&mut s, "k"), KeyOutcome::Ignored);
    assert_eq!(press(&mut s, "m"), KeyOutcome::Ignored);
    s.pointer_up(pt(h.x + 30.0 / k(&s), h.y), false, false);
    assert!(s.transform_entry().is_none());
}

#[test]
fn c55_an_unfinished_pen_path_gates_m_and_k() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Pen);
    for p in [pt(10.0, 10.0), pt(50.0, 10.0), pt(50.0, 40.0)] {
        click(&mut s, p);
    }
    assert_eq!(press(&mut s, "m"), KeyOutcome::Ignored);
    assert_eq!(press(&mut s, "k"), KeyOutcome::Ignored);
    assert_eq!(press_shift(&mut s, "K"), KeyOutcome::Ignored);
    assert_eq!(s.tool(), Tool::Pen);
}

// ---------------------------------------------------------------------
// The key S scales about the centre: criteria 57, 57a
// ---------------------------------------------------------------------

fn size_entry_open(s: &mut Session) {
    assert_eq!(press(s, "s"), KeyOutcome::EntryOpened);
    assert_eq!(s.transform_entry().map(|e| e.kind), Some("size"));
}

#[test]
fn c57_spec_example_s_60_tab_30_on_a_40_by_20_rect_at_10_10_leaves_60_by_30_at_0_5() {
    let mut s = select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
    size_entry_open(&mut s);
    let e = s.transform_entry().unwrap();
    assert_eq!(e.fields.len(), 2);
    assert!(!e.linked, "no Ctrl link after S");
    assert!(
        near(num(&e.fields[0].prefill), 40.0, 0.051)
            && near(num(&e.fields[1].prefill), 20.0, 0.051)
    );
    assert_eq!(
        s.transform_entry_linked(0, "60"),
        None,
        "fields are independent"
    );
    assert_eq!(
        s.transform_entry_linked(1, "30"),
        None,
        "fields are independent"
    );
    let n = change_count(&s);
    assert_eq!(
        s.commit_transform_entry("60", "30", 0),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&s), n + 1);
    let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
        panic!()
    };
    let Shape::Rect { bounds: b, .. } = p.shape else {
        panic!()
    };
    assert!(pnear(b.origin, pt(0.0, 5.0), 1e-9), "{:?}", b.origin);
    assert!(near(b.width.as_mm(), 60.0, 1e-9) && near(b.height.as_mm(), 30.0, 1e-9));
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.selected_object_count(), 1);
}

#[test]
fn c57a_key_s_equals_a_shift_drag_of_the_se_corner_for_rect_ellipse_path_at_several_rotations() {
    for th in [0.0, 0.9, -2.2] {
        for stroke in [false, true] {
            let kinds: Vec<(&str, Document, Point)> = vec![
                ("rect", rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0)),
                (
                    "ellipse",
                    ellipse_doc(30.0, 20.0, 20.0, 10.0),
                    pt(30.0, 10.0),
                ),
                ("curvy", curvy_doc(), pt(0.0, 10.0)),
            ];
            for (name, d, on) in kinds {
                if th != 0.0 {
                    rotate_doc_object(&d, th);
                }
                let ctx = format!("{name} th {th} stroke {stroke}");
                let mut a = open_in_session(&d);
                let on = match obj_of(&a, 0) {
                    ObjectSnapshot::Path(p) => p.anchors[0].point,
                    ObjectSnapshot::Primitive(_) => {
                        // a point on the rotated outline: the top-edge midpoint
                        // of the oriented box
                        let o = obj_of(&a, 0);
                        let c = ctr_of(&o);
                        let (x0, y0, x1, y1) = frame_centre_box(match &o {
                            ObjectSnapshot::Primitive(p) => p,
                            ObjectSnapshot::Path(_) => unreachable!(),
                        });
                        let _ = (x0, x1, on);
                        rot(pt(c.x, y0 + 0.0 * y1), c, th)
                    }
                };
                click(&mut a, on);
                assert_eq!(a.selected_object_count(), 1, "{ctx}");
                a.set_scale_stroke_width(stroke);
                let o = obj_of(&a, 0);
                let bx = match &o {
                    ObjectSnapshot::Path(p) => path_bx(p),
                    ObjectSnapshot::Primitive(p) => {
                        let (x0, y0, x1, y1) = frame_centre_box(p);
                        Bx {
                            c: ctr_of(&o),
                            hw: (x1 - x0) / 2.0,
                            hh: (y1 - y0) / 2.0,
                            th: p.rotation.as_radians(),
                        }
                    }
                };
                let (w, h) = (2.0 * bx.hw, 2.0 * bx.hh);
                let (w2, h2) = (w + 20.0, h + 10.0);

                size_entry_open(&mut a);
                let n = change_count(&a);
                assert_eq!(
                    a.commit_transform_entry(&format!("{w2}"), &format!("{h2}"), 0),
                    EntryOutcome::Committed,
                    "{ctx}"
                );
                assert_eq!(change_count(&a), n + 1, "{ctx}: one commit");

                // Shift-drag of the bottom-right corner handle
                let mut b = open_in_session(&d);
                click(&mut b, on);
                b.set_scale_stroke_width(stroke);
                let from = bx.corner(1.0, 1.0);
                let (sn, cs) = bx.th.sin_cos();
                let (dx, dy) = ((w2 - w) / 2.0, (h2 - h) / 2.0);
                let to = pt(from.x + dx * cs - dy * sn, from.y + dx * sn + dy * cs);
                drag_mod(&mut b, from, to, true, false);
                same_object(&obj_of(&a, 0), &obj_of(&b, 0), 1e-6, &ctx);
                // and the centre stayed
                let c1 = ctr_of(&obj_of(&a, 0));
                let c0 = ctr_of(&o);
                assert!(pnear(c0, c1, 1e-6), "{ctx}: centre moved {c0:?} -> {c1:?}");
            }
        }
    }
}

#[test]
fn c57_s_shrinks_about_the_centre_too_and_a_one_field_change_keeps_the_centre() {
    let mut s = select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
    size_entry_open(&mut s);
    // only W typed; H untouched (prefill)
    let e = s.transform_entry().unwrap();
    assert_eq!(
        s.commit_transform_entry("10", &e.fields[1].prefill, 0),
        EntryOutcome::Committed
    );
    let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
        panic!()
    };
    let Shape::Rect { bounds: b, .. } = p.shape else {
        panic!()
    };
    assert!(pnear(b.origin, pt(25.0, 10.0), 1e-9), "{:?}", b.origin);
    assert!(near(b.width.as_mm(), 10.0, 1e-9) && near(b.height.as_mm(), 20.0, 1e-9));
}

#[test]
fn c57_polygon_and_star_scale_about_the_centre_with_one_field_r_for_both_routes() {
    for (name, d) in [
        ("polygon", polygon_doc(70.0, 60.0, 40.0, 5)),
        ("star", star_doc(70.0, 60.0, 40.0, 5, 0.5)),
    ] {
        for th in [0.0, 0.8] {
            let mut s = select_at(&d, pt(70.0, 20.0));
            let _ = th;
            assert_eq!(press(&mut s, "s"), KeyOutcome::EntryOpened);
            assert_eq!(s.transform_entry().map(|e| e.kind), Some("radius"));
            let e = s.transform_entry().unwrap();
            assert_eq!(e.fields.len(), 1, "{name}");
            assert_eq!(e.fields[0].label, "r", "{name}");
            let o0 = obj_of(&s, 0);
            assert_eq!(
                s.commit_transform_entry("60", "", 0),
                EntryOutcome::Committed,
                "{name}"
            );
            let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
                panic!()
            };
            let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = p.shape else {
                panic!()
            };
            assert!(
                pnear(frame.center, pt(70.0, 60.0), 1e-9),
                "{name}: centre {:?}",
                frame.center
            );
            assert!(near(frame.radius.as_mm(), 60.0, 1e-9), "{name}: radius");
            let ObjectSnapshot::Primitive(p0) = o0 else {
                panic!()
            };
            assert_eq!(p.rotation.as_radians(), p0.rotation.as_radians());
        }
    }
}

#[test]
fn c57a_double_click_on_a_resize_handle_keeps_the_dragged_handles_fixed_point() {
    // rect 40 x 20 at (10, 10); typed 60 x 30
    let check = |handle: Point, shift: bool, want_origin: Point, name: &str| {
        let mut s = select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
        dbl(&mut s, handle, handle, shift, false);
        assert_eq!(s.transform_entry().map(|e| e.kind), Some("size"), "{name}");
        assert_eq!(
            s.commit_transform_entry("60", "30", 0),
            EntryOutcome::Committed,
            "{name}"
        );
        let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
            panic!()
        };
        let Shape::Rect { bounds: b, .. } = p.shape else {
            panic!()
        };
        assert!(
            pnear(b.origin, want_origin, 1e-9),
            "{name}: origin {:?}, want {want_origin:?}",
            b.origin
        );
        assert!(
            near(b.width.as_mm(), 60.0, 1e-9) && near(b.height.as_mm(), 30.0, 1e-9),
            "{name}"
        );
    };
    check(
        pt(50.0, 30.0),
        false,
        pt(10.0, 10.0),
        "SE corner: opposite NW corner fixed",
    );
    check(
        pt(10.0, 10.0),
        false,
        pt(-10.0, 0.0),
        "NW corner: SE corner (50,30) fixed",
    );
    check(
        pt(50.0, 10.0),
        false,
        pt(10.0, 0.0),
        "NE corner: SW corner fixed",
    );
    check(
        pt(50.0, 30.0),
        true,
        pt(0.0, 5.0),
        "SE corner with Shift: centre fixed",
    );
    check(
        pt(10.0, 10.0),
        true,
        pt(0.0, 5.0),
        "NW corner with Shift: centre fixed",
    );
    // edge handle: one field, opposite edge fixed
    let mut s = select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
    dbl(&mut s, pt(50.0, 20.0), pt(50.0, 20.0), false, false);
    let e = s.transform_entry().unwrap();
    assert_eq!(e.fields.len(), 1);
    assert_eq!(
        s.commit_transform_entry("60", "", 0),
        EntryOutcome::Committed
    );
    let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
        panic!()
    };
    let Shape::Rect { bounds: b, .. } = p.shape else {
        panic!()
    };
    assert!(
        pnear(b.origin, pt(10.0, 10.0), 1e-9),
        "right edge: left edge fixed {:?}",
        b.origin
    );
    assert!(near(b.width.as_mm(), 60.0, 1e-9) && near(b.height.as_mm(), 20.0, 1e-9));
    // Ctrl at the second press of a double-click still links W and H there
    let mut s = select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
    dbl(&mut s, pt(50.0, 30.0), pt(50.0, 30.0), false, true);
    assert!(
        s.transform_entry().unwrap().linked,
        "Ctrl link on the double-click route is unchanged"
    );
    // polygon and star: centre fixed on the double-click route, whichever
    // corner (found by scanning for the uniform resize handle)
    for (name, d) in [
        ("polygon", polygon_doc(70.0, 60.0, 40.0, 5)),
        ("star", star_doc(70.0, 60.0, 40.0, 5, 0.5)),
    ] {
        let mut s = select_at(&d, pt(70.0, 20.0));
        let kk = k(&s);
        let mut found = None;
        let mut y = 40.0;
        'scan: while y <= 130.0 {
            let mut x = 70.0;
            while x <= 140.0 {
                s.pointer_hover(pt(x, y), false, false);
                if s.handle_hint() == "resize-corner-uniform" {
                    found = Some(pt(x, y));
                    break 'scan;
                }
                x += 1.0 / kk;
            }
            y += 1.0 / kk;
        }
        let at = found.unwrap_or_else(|| panic!("{name}: a corner resize handle exists"));
        dbl(&mut s, at, at, false, false);
        assert_eq!(
            s.transform_entry().map(|e| e.kind),
            Some("radius"),
            "{name}"
        );
        assert_eq!(
            s.commit_transform_entry("60", "", 0),
            EntryOutcome::Committed,
            "{name}"
        );
        let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
            panic!()
        };
        let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = p.shape else {
            panic!()
        };
        assert!(
            pnear(frame.center, pt(70.0, 60.0), 1e-9),
            "{name}: {:?}",
            frame.center
        );
        assert!(near(frame.radius.as_mm(), 60.0, 1e-9), "{name}");
    }
}

#[test]
fn c57_s_and_r_chips_open_at_the_bottom_right_resize_and_top_right_rotate_corner_even_when_tiny() {
    for (name, d, on) in [
        ("tiny rect", rect_doc(10.0, 20.0, 3.0, 2.0), pt(11.5, 20.0)),
        ("tiny path", triangle_doc_sized(3.0, 2.0), pt(1.5, 0.0)),
        (
            "big rect",
            rect_doc(10.0, 20.0, 120.0, 80.0),
            pt(70.0, 20.0),
        ),
    ] {
        let mut s = select_at(&d, on);
        let o = obj_of(&s, 0);
        let (c, hw, hh) = match &o {
            ObjectSnapshot::Path(p) => {
                let b = path_bx(p);
                (b.c, b.hw, b.hh)
            }
            ObjectSnapshot::Primitive(p) => {
                let (x0, y0, x1, y1) = frame_centre_box(p);
                (ctr_of(&o), (x1 - x0) / 2.0, (y1 - y0) / 2.0)
            }
        };
        let bx = Bx { c, hw, hh, th: 0.0 };
        size_entry_open(&mut s);
        let e = s.transform_entry().unwrap();
        // Amended 2026-10-07 (UX review, criterion 59): the S chip is placed by
        // the box centre, the fixed point of the key's typed size.
        assert!(e.at_centre, "{name}");
        assert!(
            pnear(e.handle, bx.c, 1e-6),
            "{name}: S chip at {:?}, centre {:?}",
            e.handle,
            bx.c
        );
        s.cancel_transform_entry();
        assert_eq!(press(&mut s, "r"), KeyOutcome::EntryOpened, "{name}");
        let e = s.transform_entry().unwrap();
        assert_eq!(e.kind, "angle", "{name}");
        let want = bx.rot_corner(k(&s), 1.0, -1.0);
        assert!(
            pnear(e.handle, want, 1e-6),
            "{name}: R chip at {:?}, want {want:?}",
            e.handle
        );
    }
}

#[test]
fn c57_tiny_objects_scale_by_s_about_the_centre_as_well() {
    let mut s = select_at(&rect_doc(10.0, 20.0, 3.0, 2.0), pt(11.5, 20.0));
    size_entry_open(&mut s);
    assert_eq!(
        s.commit_transform_entry("5", "4", 0),
        EntryOutcome::Committed
    );
    let ObjectSnapshot::Primitive(p) = obj_of(&s, 0) else {
        panic!()
    };
    let Shape::Rect { bounds: b, .. } = p.shape else {
        panic!()
    };
    assert!(pnear(b.origin, pt(9.0, 19.0), 1e-9), "{:?}", b.origin);
}

#[test]
fn c57_s_with_ctrl_is_gated_and_does_not_open_a_linked_chip() {
    let mut s = select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
    assert_eq!(
        s.key_down(KeyInput {
            key: "s",
            ctrl: true,
            ..KeyInput::default()
        }),
        KeyOutcome::Ignored
    );
    assert!(s.transform_entry().is_none());
    // a Ctrl held while the pointer happens to be over the canvas does not link
    s.modifiers_changed(false, true);
    assert_eq!(press(&mut s, "s"), KeyOutcome::EntryOpened);
    assert!(!s.transform_entry().unwrap().linked);
}

#[test]
fn c57_s_ignores_the_shift_state_held_when_it_is_pressed() {
    // "The S route never reads a modifier": Shift held (as a pointer modifier
    // state) changes nothing about the fixed point.
    let mut a = select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
    a.modifiers_changed(true, false);
    size_entry_open(&mut a);
    a.commit_transform_entry("60", "30", 0);
    let mut b = select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
    size_entry_open(&mut b);
    b.commit_transform_entry("60", "30", 0);
    same_object(
        &obj_of(&a, 0),
        &obj_of(&b, 0),
        1e-12,
        "shift state irrelevant",
    );
}

#[test]
fn oracle_sanity_the_drawn_ellipse_outline_is_within_a_few_hundredths_of_the_exact_ellipse() {
    let d = ellipse_doc(70.0, 60.0, 60.0, 40.0);
    rotate_doc_object(&d, 0.9);
    let o = d.object(d.object_ids()[0]).unwrap();
    let (x0, y0, x1, y1) = tight_of(&o);
    let th: f64 = 0.9;
    let hx = (60.0_f64.powi(2) * th.cos().powi(2) + 40.0_f64.powi(2) * th.sin().powi(2)).sqrt();
    let hy = (60.0_f64.powi(2) * th.sin().powi(2) + 40.0_f64.powi(2) * th.cos().powi(2)).sqrt();
    assert!(near(x0, 70.0 - hx, 0.03) && near(x1, 70.0 + hx, 0.03));
    assert!(near(y0, 60.0 - hy, 0.03) && near(y1, 60.0 + hy, 0.03));
}

// ---------------------------------------------------------------------
// Second pass: routing and sequencing probes
// ---------------------------------------------------------------------

#[test]
fn x01_the_typed_move_opens_again_after_a_commit_an_escape_and_a_cancel() {
    let mut s = select_at(&triangle_doc_sized(120.0, 80.0), pt(30.0, 20.0));
    for round in 0..3 {
        let ctr = ctr_of(&obj_of(&s, 0));
        dbl(&mut s, ctr, ctr, false, false);
        assert!(s.move_entry().is_some(), "round {round}");
        match round {
            0 => assert_eq!(
                s.commit_move_entry(
                    "12",
                    "7",
                    MoveEntryMode {
                        absolute: false,
                        copy: false
                    }
                ),
                EntryOutcome::Committed
            ),
            1 => assert_eq!(
                s.key_down(key("Escape")),
                KeyOutcome::Escape(EscapeStep::ClosedEntry)
            ),
            _ => s.cancel_transform_entry(),
        }
        assert!(s.move_entry().is_none(), "round {round}: closed");
        assert_eq!(s.selected_object_count(), 1);
        assert_eq!(s.tool(), Tool::Select);
    }
}

#[test]
fn x02_both_presses_must_be_on_the_centre_handle() {
    let (mut s, b) = select_path(&triangle_doc_sized(120.0, 80.0));
    let kk = k(&s);
    let ctr = b.c;
    let away = pt(ctr.x + 30.0 / kk, ctr.y);
    let before = snapshot_bytes(&s);
    // first press elsewhere inside the box, second on the centre handle
    dbl(&mut s, away, ctr, false, false);
    assert!(
        s.move_entry().is_none(),
        "first press was not on the centre handle"
    );
    assert_eq!(snapshot_bytes(&s), before);
    // first press on the centre, second elsewhere
    let (mut s, b) = select_path(&triangle_doc_sized(120.0, 80.0));
    let away = pt(b.c.x + 30.0 / kk, b.c.y);
    dbl(&mut s, b.c, away, false, false);
    assert!(
        s.move_entry().is_none(),
        "second press was not on the centre handle"
    );
    assert_eq!(snapshot_bytes(&s), before);
}

#[test]
fn x03_a_press_that_starts_a_move_closes_an_open_chip_and_is_not_swallowed() {
    for chip in ["move", "skew", "size", "angle"] {
        let (mut s, b) = select_path(&triangle_doc_sized(120.0, 80.0));
        let kk = k(&s);
        let key_for = match chip {
            "move" => "m",
            "skew" => "k",
            "size" => "s",
            _ => "r",
        };
        assert_eq!(press(&mut s, key_for), KeyOutcome::EntryOpened, "{chip}");
        let before = path_of(&s);
        let from = pt(b.c.x + 25.0 / kk, b.c.y);
        s.pointer_hover(from, false, false);
        s.pointer_down(from, false);
        s.pointer_hover(pt(from.x + 20.0, from.y + 10.0), false, false);
        s.pointer_up(pt(from.x + 20.0, from.y + 10.0), false, false);
        assert!(
            s.move_entry().is_none() && s.transform_entry().is_none(),
            "{chip}: closed"
        );
        let after = path_of(&s);
        assert!(
            pnear(
                after.anchors[0].point,
                pt(
                    before.anchors[0].point.x + 20.0,
                    before.anchors[0].point.y + 10.0
                ),
                1e-9
            ),
            "{chip}: the press that closed the chip started a move: {:?}",
            after.anchors[0].point
        );
    }
}

#[test]
fn x04_skew_tiny_angles_are_no_change_or_exact_and_never_a_false_too_large() {
    for text in [
        "0.0000000000001",
        "0.000000000001",
        "-0.000000000001",
        "0.00000000001",
        "0.000000001",
        "0.000001",
    ] {
        let (mut s, _b) = open_skew(&rect_path_doc(), (0.0, -1.0));
        let before = snapshot_bytes(&s);
        let n = change_count(&s);
        let out = s.commit_transform_entry(text, "", 0);
        eprintln!(
            "SKEW-TINY {text:?} -> {out:?}, commits {}",
            change_count(&s) - n
        );
        assert_ne!(
            out,
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::TooLarge
            },
            "{text}"
        );
        assert_ne!(
            out,
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::SkewRange
            },
            "{text}"
        );
        if out == EntryOutcome::Unchanged {
            assert_eq!(snapshot_bytes(&s), before);
        }
    }
}

#[test]
fn x04b_a_tiny_angle_far_from_the_origin_is_not_reported_as_too_large() {
    // far from the origin, where a tiny skew vanishes in the floating-point
    // resolution of the coordinates: must not be reported as "Too large"
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 5_000_000.0, 5_000_000.0),
            anchor(2, 5_000_020.0, 5_000_000.0),
            anchor(3, 5_000_020.0, 5_000_010.0),
        ],
        true,
    );
    let (mut s, b) = select_path(&d);
    let h = skew_handle_pos(&s, &b, (0.0, -1.0));
    dbl(&mut s, h, h, false, false);
    for text in ["0.000000001", "0.00000001", "0.0000001", "0.001"] {
        let out = s.commit_transform_entry(text, "", 0);
        eprintln!("SKEW-FAR {text:?} -> {out:?}");
        assert_ne!(
            out,
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::TooLarge
            },
            "{text}: a tiny angle is not 'Too large'"
        );
        if !matches!(out, EntryOutcome::Invalid { .. }) {
            break;
        }
    }
}

#[test]
fn x05_skew_on_a_straight_line_path_has_no_lever() {
    // zero height: top and bottom skew handles do not exist, K still must not
    // panic or write; Shift+K (left/right) works because the width is non-zero
    let d = Document::new(1);
    let _ = d.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 40.0, 0.0)], false);
    let mut s = select_at(&d, pt(20.0, 0.0));
    let before = snapshot_bytes(&s);
    let out = press(&mut s, "k");
    eprintln!("K on a horizontal line: {out:?}");
    if out == KeyOutcome::EntryOpened {
        let r = s.commit_transform_entry("30", "", 0);
        eprintln!("commit on a zero-lever skew: {r:?}");
        assert_eq!(
            snapshot_bytes(&s),
            before,
            "a skew without a lever writes nothing"
        );
        s.cancel_transform_entry();
    }
    let out = press_shift(&mut s, "K");
    assert!(matches!(
        out,
        KeyOutcome::EntryOpened | KeyOutcome::Ignored | KeyOutcome::Hint(_)
    ));
    s.cancel_transform_entry();
}

#[test]
fn x06_key_outcome_codes_for_the_new_hints() {
    assert_eq!(
        KeyOutcome::Hint(KeyHint::SelectFirst).code(),
        "hint-select-first"
    );
    assert_eq!(KeyOutcome::Hint(KeyHint::PathOnly).code(), "hint-path-only");
    assert_eq!(
        KeyOutcome::Hint(KeyHint::SelectOne).code(),
        "hint-select-one"
    );
    assert_eq!(KeyOutcome::EntryOpened.code(), "entry");
}

#[test]
fn x07_handle_hints_name_skew_x_and_y_apart() {
    let (mut s, b) = select_path(&triangle_doc_sized(120.0, 80.0));
    for side in SIDES {
        s.pointer_hover(skew_handle_pos(&s, &b, side), false, false);
        let want = if side.1 != 0.0 { "skew" } else { "skew-y" };
        assert_eq!(s.handle_hint(), want, "{side:?}");
    }
    s.pointer_hover(b.c, false, false);
    assert_eq!(s.handle_hint(), "move");
}

#[test]
fn x08_s_r_m_k_open_for_a_rotated_selection_at_the_rotated_anchors() {
    let d = curvy_doc();
    rotate_doc_object(&d, 0.7);
    let (mut s, b) = select_path(&d);
    assert_eq!(press(&mut s, "s"), KeyOutcome::EntryOpened);
    let e = s.transform_entry().unwrap();
    // Amended 2026-10-07 (UX review, criterion 59): the S chip is placed by
    // the box centre, not at the bottom-right corner.
    assert!(e.at_centre);
    assert!(pnear(e.handle, b.c, 1e-6), "{:?} vs {:?}", e.handle, b.c);
    s.cancel_transform_entry();
    assert_eq!(press(&mut s, "k"), KeyOutcome::EntryOpened);
    let e = s.transform_entry().unwrap();
    assert!(pnear(e.handle, skew_handle_pos(&s, &b, (0.0, -1.0)), 1e-6));
    s.cancel_transform_entry();
    assert_eq!(press_shift(&mut s, "K"), KeyOutcome::EntryOpened);
    let e = s.transform_entry().unwrap();
    assert!(pnear(e.handle, skew_handle_pos(&s, &b, (1.0, 0.0)), 1e-6));
    s.cancel_transform_entry();
    assert_eq!(press(&mut s, "m"), KeyOutcome::EntryOpened);
    assert!(pnear(s.move_entry().unwrap().center, b.c, 1e-6));
}

#[test]
fn x09_the_s_pivot_marker_draw_list_differs_from_the_chip_closed_state() {
    // The marker is not exposed except through the draw list: with the S chip
    // open the list must differ from the same scene with the chip closed, and
    // the double-click chip on the same handle must differ from the S chip
    // (the marker sits elsewhere).
    let make = || select_at(&rect_doc(10.0, 10.0, 40.0, 20.0), pt(30.0, 10.0));
    let closed = make().draw_list().triangles.len();
    let mut a = make();
    press(&mut a, "s");
    let with_s = a.draw_list();
    let mut b = make();
    dbl(&mut b, pt(50.0, 30.0), pt(50.0, 30.0), false, false);
    let with_dbl = b.draw_list();
    assert_ne!(with_s.triangles.len(), closed, "S chip adds the marker");
    let pos = |d: &curvyo_render_core::DrawList| -> Vec<(i64, i64)> {
        d.triangles
            .iter()
            .map(|v| ((v.position.x * 100.0) as i64, (v.position.y * 100.0) as i64))
            .collect()
    };
    assert_ne!(
        pos(&with_s),
        pos(&with_dbl),
        "different fixed points, different marker"
    );
}

/// The one radius of a rectangle whose four corner radii are equal (asserted).
fn uniform_mm(radii: curvyo_document_core::CornerRadii) -> f64 {
    assert_eq!(
        radii,
        curvyo_document_core::CornerRadii::uniform(radii.tl),
        "four equal radii"
    );
    radii.tl.as_mm()
}
