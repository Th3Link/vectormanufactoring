//! Independent black-box tests for `specs/0005-object-transform/
//! specification.md` AC 8 and AC 26-31 (the "Scale stroke width" switch),
//! written from the spec before reading the implementation. They drive the
//! public `Session` API only and read results back out of a packed file.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::match_same_arms,
    clippy::needless_range_loop
)]

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};
use std::io::{Cursor, Write};

use curvyo_document_core::{
    AnchorId, Angle, CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Document, EllipseFrame,
    InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot, Point, PointCount, RectBounds, Shape,
    StarFrame, Vec2, outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};

const EPS: f64 = 1e-9;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

/// Own rotation formula (y-down, positive = clockwise on screen).
fn rot(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

fn rb(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    }
}

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Kind {
    Rect,
    RotatedRect,
    Ellipse,
    Polygon,
    Star,
    OpenPath,
    ClosedPath,
}

const ALL_KINDS: [Kind; 7] = [
    Kind::Rect,
    Kind::RotatedRect,
    Kind::Ellipse,
    Kind::Polygon,
    Kind::Star,
    Kind::OpenPath,
    Kind::ClosedPath,
];

const ROT: f64 = 0.5;

impl Kind {
    /// Polygon and star only offer corner handles (AC 11).
    fn corners_only(self) -> bool {
        matches!(self, Kind::Polygon | Kind::Star)
    }

    /// Local-frame bounding box `(x0, y0, x1, y1)` the handles sit on.
    fn bbox(self) -> (f64, f64, f64, f64) {
        match self {
            Kind::Polygon | Kind::Star => (20.0, 20.0, 40.0, 40.0),
            _ => (0.0, 0.0, 40.0, 20.0),
        }
    }

    fn rotation(self) -> f64 {
        if matches!(self, Kind::RotatedRect) {
            ROT
        } else {
            0.0
        }
    }

    fn centre(self) -> Point {
        let (x0, y0, x1, y1) = self.bbox();
        pt(f64::midpoint(x0, x1), f64::midpoint(y0, y1))
    }
}

fn path_anchors(closed: bool) -> Vec<NewAnchor> {
    if closed {
        vec![
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(40.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(40.0, 20.0)),
        ]
    } else {
        vec![
            NewAnchor {
                handle_out: Vec2::new(10.0, 4.0),
                ..NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0))
            },
            NewAnchor {
                handle_in: Vec2::new(-10.0, -4.0),
                ..NewAnchor::corner(AnchorId::new(1, 2), pt(40.0, 20.0))
            },
        ]
    }
}

/// Builds one object of `kind`; `stroke` (mm) is written when given, else
/// the model default stays.
fn build(kind: Kind, stroke: Option<f64>) -> (Document, NodeId) {
    let d = Document::new(1);
    let w = stroke.map(Length::from_mm);
    let id = match kind {
        Kind::Rect | Kind::RotatedRect => {
            let id = d.create_rect(rb(0.0, 0.0, 40.0, 20.0));
            if w.is_some() {
                d.resize_rect(
                    id,
                    rb(0.0, 0.0, 40.0, 20.0),
                    curvyo_document_core::CornerRadii::uniform(Length::from_mm(0.0)),
                    w,
                )
                .unwrap();
            }
            id
        }
        Kind::Ellipse => {
            let f = EllipseFrame {
                center: pt(20.0, 10.0),
                rx: Length::from_mm(20.0),
                ry: Length::from_mm(10.0),
            };
            let id = d.create_ellipse(f);
            if w.is_some() {
                d.resize_ellipse(id, f, w).unwrap();
            }
            id
        }
        Kind::Polygon | Kind::Star => {
            let f = StarFrame {
                center: pt(30.0, 30.0),
                radius: Length::from_mm(10.0),
                angle: Angle::from_radians(-FRAC_PI_2),
            };
            let id = if matches!(kind, Kind::Polygon) {
                d.create_polygon(f, PointCount::new(6).unwrap())
            } else {
                d.create_star(
                    f,
                    PointCount::new(5).unwrap(),
                    InnerRatio::new(0.5).unwrap(),
                )
            };
            if w.is_some() {
                d.resize_star_frame(id, f, w).unwrap();
            }
            id
        }
        Kind::OpenPath | Kind::ClosedPath => {
            let closed = matches!(kind, Kind::ClosedPath);
            let id = d.create_path(&path_anchors(closed), closed);
            if w.is_some() {
                d.resize_path(id, &[], w).unwrap();
            }
            id
        }
    };
    if matches!(kind, Kind::RotatedRect) {
        let rotated = d
            .object(id)
            .unwrap()
            .rotated(pt(20.0, 10.0), Angle::from_radians(ROT));
        d.rotate_object(&rotated).unwrap();
    }
    (d, id)
}

fn open(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).expect("opens");
    s.set_tool(Tool::Select);
    s
}

fn snapshot(s: &Session) -> ObjectSnapshot {
    let d = unpack(99, &s.pack("0.1.0").unwrap()).unwrap();
    let ids = d.object_ids();
    d.object(ids[0]).unwrap()
}

fn stroke(o: &ObjectSnapshot) -> f64 {
    match o {
        ObjectSnapshot::Primitive(p) => p.style.stroke.width.as_mm(),
        ObjectSnapshot::Path(p) => p.style.stroke.width.as_mm(),
    }
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn drag(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
}

/// A point on the object's outline that selects it.
fn select_point(kind: Kind, s: &Session) -> Point {
    match kind {
        Kind::Rect => pt(20.0, 0.0),
        Kind::RotatedRect => rot(pt(20.0, 0.0), pt(20.0, 10.0), ROT),
        Kind::Ellipse => pt(20.0 + 20.0 * FRAC_1_SQRT_2, 10.0 + 10.0 * FRAC_1_SQRT_2),
        Kind::Polygon | Kind::Star => {
            let ObjectSnapshot::Primitive(p) = snapshot(s) else {
                panic!()
            };
            let o = outline_of_rotated(&p.shape, p.rotation);
            pt(
                f64::midpoint(o[0].point.x, o[1].point.x),
                f64::midpoint(o[0].point.y, o[1].point.y),
            )
        }
        Kind::OpenPath => pt(20.0, 10.0),
        Kind::ClosedPath => pt(20.0, 0.0),
    }
}

fn selected(kind: Kind, stroke_mm: Option<f64>) -> Session {
    let (d, _) = build(kind, stroke_mm);
    let mut s = open(&d);
    let p = select_point(kind, &s);
    click(&mut s, p);
    s
}

/// Local-frame handle positions, `(handle, anchor-for-that-handle,
/// horizontal-axis-active, vertical-axis-active)`.
fn handles(kind: Kind) -> Vec<(Point, Point, bool, bool)> {
    let (x0, y0, x1, y1) = kind.bbox();
    let (xm, ym) = (f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    let mut v = vec![
        (pt(x1, y1), pt(x0, y0), true, true),
        (pt(x0, y0), pt(x1, y1), true, true),
        (pt(x1, y0), pt(x0, y1), true, true),
        (pt(x0, y1), pt(x1, y0), true, true),
    ];
    if !kind.corners_only() {
        v.extend([
            (pt(xm, y0), pt(xm, y1), false, true),
            (pt(xm, y1), pt(xm, y0), false, true),
            (pt(x0, ym), pt(x1, ym), true, false),
            (pt(x1, ym), pt(x0, ym), true, false),
        ]);
    }
    v
}

/// The resize gesture: factors `(fx, fy)` along the local axes; `shift`
/// anchors at the centre. Returns `(press, release)` in document space.
fn gesture(
    kind: Kind,
    h: (Point, Point, bool, bool),
    f: (f64, f64),
    shift: bool,
) -> (Point, Point) {
    let c = kind.centre();
    let (handle, anchor, hx, vy) = h;
    let o = if shift { c } else { anchor };
    let to = pt(
        if hx {
            o.x + (handle.x - o.x) * f.0
        } else {
            handle.x
        },
        if vy {
            o.y + (handle.y - o.y) * f.1
        } else {
            handle.y
        },
    );
    let a = kind.rotation();
    (rot(handle, c, a), rot(to, c, a))
}

// ---------------------------------------------------------------------
// Loro op-log helpers: "writes no stroke key at all"
// ---------------------------------------------------------------------

fn loro_of(s: &Session) -> loro::LoroDoc {
    let d = unpack(99, &s.pack("0.1.0").unwrap()).unwrap();
    let l = loro::LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn vv(s: &Session) -> loro::VersionVector {
    loro_of(s).oplog_vv()
}

fn ops_since(s: &Session, from: &loro::VersionVector) -> String {
    let l = loro_of(s);
    format!("{:?}", l.export_json_updates(from, &l.oplog_vv()))
}

// ---------------------------------------------------------------------
// AC 8: default (off) leaves the stroke untouched for every handle kind
// ---------------------------------------------------------------------

#[test]
fn ac8_switch_off_never_touches_the_stroke_for_any_kind_handle_gesture_or_width() {
    let gestures: [((f64, f64), bool, bool); 5] = [
        ((1.5, 2.5), false, false), // free
        ((0.4, 0.6), false, false), // shrink
        ((1.5, 2.5), true, false),  // from the centre
        ((2.0, 2.0), false, true),  // Ctrl proportional (ignored for edges)
        ((3.0, 3.0), true, true),   // Shift + Ctrl
    ];
    for kind in ALL_KINDS {
        for width in [None, Some(0.01), Some(1.0), Some(3.7)] {
            for (hi, h) in handles(kind).into_iter().enumerate() {
                for (gi, (f, shift, ctrl)) in gestures.into_iter().enumerate() {
                    let mut s = selected(kind, width);
                    assert!(!s.scale_stroke_width(), "default must be off");
                    let before = snapshot(&s);
                    let from = vv(&s);
                    let (press, release) = gesture(kind, h, f, shift);
                    drag(&mut s, press, release, shift, ctrl);
                    let after = snapshot(&s);
                    let ctx = format!("{kind:?} width {width:?} handle {hi} gesture {gi}");
                    assert_ne!(
                        format!("{before:?}"),
                        format!("{after:?}"),
                        "the drag must resize ({ctx})"
                    );
                    assert_eq!(stroke(&before), stroke(&after), "{ctx}");
                    let ops = ops_since(&s, &from);
                    assert!(
                        !ops.contains("stroke_width"),
                        "no stroke key may be written ({ctx}): {ops}"
                    );
                }
            }
        }
    }
}

#[test]
fn ac8_a_rect_with_a_one_millimetre_stroke_resized_to_300_percent_keeps_one_millimetre() {
    let mut s = selected(Kind::Rect, Some(1.0));
    drag(&mut s, pt(40.0, 20.0), pt(120.0, 60.0), false, false);
    let ObjectSnapshot::Primitive(p) = snapshot(&s) else {
        panic!()
    };
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    assert!(close(bounds.width.as_mm(), 120.0) && close(bounds.height.as_mm(), 60.0));
    assert_eq!(p.style.stroke.width.as_mm(), 1.0);
}

// ---------------------------------------------------------------------
// AC 26: switch on
// ---------------------------------------------------------------------

fn resized_stroke_on(kind: Kind, width: f64, f: (f64, f64), hi: usize, shift: bool) -> (f64, f64) {
    let mut s = selected(kind, Some(width));
    s.set_scale_stroke_width(true);
    assert!(s.scale_stroke_width());
    let h = handles(kind)[hi];
    let (press, release) = gesture(kind, h, f, shift);
    drag(&mut s, press, release, shift, false);
    (stroke(&snapshot(&s)), width)
}

/// Polygon/star: the resize is one uniform factor, whichever point stays
/// fixed (AC 11 does not pin it); returns (stroke ratio, radius ratio).
fn poly_ratios(kind: Kind, width: f64, hi: usize, shift: bool, f: f64) -> (f64, f64) {
    let mut s = selected(kind, Some(width));
    s.set_scale_stroke_width(true);
    let h = handles(kind)[hi];
    let (press, release) = gesture(kind, h, (f, f), shift);
    drag(&mut s, press, release, shift, false);
    let ObjectSnapshot::Primitive(p) = snapshot(&s) else {
        panic!()
    };
    let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = p.shape else {
        panic!()
    };
    (p.style.stroke.width.as_mm() / width, frame.radius.as_mm() / 10.0)
}

#[test]
fn ac26_equal_factors_scale_the_stroke_by_that_factor() {
    for kind in [Kind::Polygon, Kind::Star] {
        for hi in 0..4 {
            for shift in [false, true] {
                for f in [1.5, 0.5, 3.0] {
                    let (sr, rr) = poly_ratios(kind, 1.0, hi, shift, f);
                    assert!(
                        close(sr, rr.max(0.01)),
                        "{kind:?} corner {hi} shift {shift} f {f}: stroke x{sr}, radius x{rr}"
                    );
                    assert!(rr > 0.0 && (rr - 1.0).abs() > 0.1, "must resize: {rr}");
                }
            }
        }
    }
    for kind in [
        Kind::Rect,
        Kind::RotatedRect,
        Kind::Ellipse,
        Kind::OpenPath,
        Kind::ClosedPath,
    ] {
        for width in [0.5, 1.0, 3.7] {
            for (hi, _) in handles(kind).iter().enumerate().take(4) {
                let (got, w) = resized_stroke_on(kind, width, (1.5, 1.5), hi, false);
                assert!(close(got, w * 1.5), "{kind:?} w {w} corner {hi}: {got}");
                let (got, w) = resized_stroke_on(kind, width, (2.0, 2.0), hi, true);
                assert!(close(got, w * 2.0), "{kind:?} centre {hi}: {got}");
            }
        }
    }
}

#[test]
fn ac26_different_factors_and_edge_handles_use_the_geometric_mean() {
    for kind in [
        Kind::Rect,
        Kind::RotatedRect,
        Kind::Ellipse,
        Kind::OpenPath,
        Kind::ClosedPath,
    ] {
        let hs = handles(kind);
        for width in [0.5, 2.0] {
            // Free corner resize, sx != sy.
            for hi in 0..4 {
                let (got, w) = resized_stroke_on(kind, width, (1.5, 2.5), hi, false);
                assert!(
                    close(got, w * (1.5_f64 * 2.5).sqrt()),
                    "{kind:?} corner {hi}: {got}"
                );
            }
            // Edge handles: only one axis scales, the other factor is 1.
            for hi in 4..hs.len() {
                let (got, w) = resized_stroke_on(kind, width, (2.0, 3.0), hi, false);
                let f = if hs[hi].2 { 2.0 } else { 3.0 };
                assert!(
                    close(got, w * f64::sqrt(f)),
                    "{kind:?} edge {hi}: {got} vs {}",
                    w * f64::sqrt(f)
                );
            }
        }
    }
}

#[test]
fn ac26_the_stroke_never_drops_below_one_hundredth_of_a_millimetre() {
    for kind in [Kind::Rect, Kind::Ellipse, Kind::OpenPath, Kind::RotatedRect] {
        for width in [0.01, 0.25, 3.0] {
            for hi in 0..8 {
                // Tiny positive scale, exactly zero, and past the anchor.
                for f in [
                    (0.001, 0.001),
                    (0.0, 0.0),
                    (-1.5, -1.5),
                    (-1.0, 1.0),
                    (1.0, -2.0),
                ] {
                    let (got, _) = resized_stroke_on(kind, width, f, hi, false);
                    assert!(
                        got.is_finite() && got >= 0.01 - 1e-12,
                        "{kind:?} w {width} handle {hi} f {f:?}: {got}"
                    );
                }
            }
        }
    }
    // The floor is exactly 0.01 where the rule gives less.
    let (got, _) = resized_stroke_on(Kind::Rect, 0.25, (0.01, 0.01), 0, false);
    assert!(close(got, 0.01), "{got}");
}

#[test]
fn ac26_a_collapsed_or_flipped_resize_still_yields_a_file_that_opens() {
    for kind in ALL_KINDS {
        for f in [(0.0, 0.0), (-2.0, -2.0), (-1.0, 3.0)] {
            let mut s = selected(kind, Some(1.0));
            s.set_scale_stroke_width(true);
            let h = handles(kind)[0];
            let (press, release) = gesture(kind, h, f, false);
            drag(&mut s, press, release, false, false);
            let bytes = s.pack("0.1.0").unwrap();
            let d = unpack(7, &bytes).unwrap_or_else(|e| panic!("{kind:?} {f:?}: {e:?}"));
            let w = stroke(&d.object(d.object_ids()[0]).unwrap());
            assert!(w.is_finite() && w >= 0.01 - 1e-12, "{kind:?} {f:?}: {w}");
        }
    }
}

#[test]
fn ac26_huge_pointer_coordinates_keep_the_stroke_finite_and_positive() {
    for to in [1e9, 1e30, 1e300, f64::MAX] {
        let mut s = selected(Kind::Rect, Some(3.7));
        s.set_scale_stroke_width(true);
        drag(&mut s, pt(40.0, 20.0), pt(to, to), false, false);
        let bytes = s.pack("0.1.0").unwrap();
        let d = unpack(7, &bytes).expect("opens");
        let w = stroke(&d.object(d.object_ids()[0]).unwrap());
        assert!(w.is_finite() && w >= 0.01 - 1e-12, "to {to}: {w}");
    }
}

#[test]
fn ac26_switch_on_writes_only_the_stroke_width_key_besides_the_geometry() {
    let mut s = selected(Kind::Rect, Some(1.0));
    s.set_scale_stroke_width(true);
    let from = vv(&s);
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 30.0), false, false);
    let ops = ops_since(&s, &from);
    assert!(
        ops.contains("stroke_width") && ops.contains("rect_bounds"),
        "{ops}"
    );
    for other in ["dash", "opacity", "fill", "\"stroke\"", "rotation"] {
        assert!(!ops.contains(other), "{other} written: {ops}");
    }
}

// ---------------------------------------------------------------------
// AC 27: every session starts off
// ---------------------------------------------------------------------

#[test]
fn ac27_new_and_opened_sessions_start_off_even_after_another_was_on() {
    let mut first = Session::new(1);
    assert!(!first.scale_stroke_width());
    first.set_scale_stroke_width(true);
    assert!(first.scale_stroke_width());
    let bytes = first.pack("0.1.0").unwrap();
    // Opening the saved project in a new session: off.
    let reopened = Session::open(2, &bytes).unwrap();
    assert!(!reopened.scale_stroke_width());
    // A fresh session is off too, regardless of the previous one.
    assert!(!Session::new(3).scale_stroke_width());
}

#[test]
fn ac27_the_switch_is_not_read_from_a_project_file() {
    // A file saved with the switch on is byte-identical to one saved off
    // (AC 29), so it cannot carry the state; reopening shows off.
    let (d, _) = build(Kind::Rect, Some(1.0));
    let mut s = open(&d);
    s.set_scale_stroke_width(true);
    let reopened = Session::open(5, &s.pack("0.1.0").unwrap()).unwrap();
    assert!(!reopened.scale_stroke_width());
}

// ---------------------------------------------------------------------
// AC 28: captured at press
// ---------------------------------------------------------------------

#[test]
fn ac28_toggling_on_mid_drag_does_not_change_that_drag_and_applies_to_the_next() {
    let mut s = selected(Kind::Rect, Some(1.0));
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(60.0, 30.0), false, false);
    s.set_scale_stroke_width(true);
    s.pointer_hover(pt(80.0, 40.0), false, false);
    // live preview (if it is written to the file) must not scale the stroke
    assert_eq!(stroke(&snapshot(&s)), 1.0, "preview scaled stroke");
    s.pointer_up(pt(80.0, 40.0), false, false);
    assert_eq!(stroke(&snapshot(&s)), 1.0, "commit scaled stroke");
    assert!(s.scale_stroke_width(), "the switch state itself changed");
    // Next drag uses on: rect is now 80 x 40, corner at (80,40) -> (160,80).
    drag(&mut s, pt(80.0, 40.0), pt(160.0, 80.0), false, false);
    assert!(
        close(stroke(&snapshot(&s)), 2.0),
        "{}",
        stroke(&snapshot(&s))
    );
}

#[test]
fn ac28_toggling_off_mid_drag_does_not_change_that_drag_and_applies_to_the_next() {
    let mut s = selected(Kind::Rect, Some(1.0));
    s.set_scale_stroke_width(true);
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(60.0, 30.0), false, false);
    s.set_scale_stroke_width(false);
    s.pointer_hover(pt(80.0, 40.0), false, false);
    s.pointer_up(pt(80.0, 40.0), false, false);
    assert!(
        close(stroke(&snapshot(&s)), 2.0),
        "got {}",
        stroke(&snapshot(&s))
    );
    drag(&mut s, pt(80.0, 40.0), pt(160.0, 80.0), false, false);
    assert!(close(stroke(&snapshot(&s)), 2.0), "second drag must keep");
}

#[test]
fn ac28_toggling_twice_mid_drag_and_toggling_at_release_changes_nothing() {
    for start_on in [false, true] {
        let mut s = selected(Kind::Ellipse, Some(1.0));
        s.set_scale_stroke_width(start_on);
        let h = handles(Kind::Ellipse)[0];
        let (press, release) = gesture(Kind::Ellipse, h, (2.0, 2.0), false);
        s.pointer_hover(press, false, false);
        s.pointer_down(press, false);
        s.set_scale_stroke_width(!start_on);
        s.pointer_hover(release, false, false);
        s.set_scale_stroke_width(start_on);
        s.set_scale_stroke_width(!start_on);
        s.pointer_up(release, false, false);
        let expected = if start_on { 2.0 } else { 1.0 };
        assert!(
            close(stroke(&snapshot(&s)), expected),
            "start_on {start_on}: {}",
            stroke(&snapshot(&s))
        );
    }
}

#[test]
fn ac28_an_escaped_drag_leaves_the_stroke_and_the_next_drag_uses_the_new_state() {
    let mut s = selected(Kind::Rect, Some(1.0));
    s.set_scale_stroke_width(true);
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(80.0, 40.0), false, false);
    s.escape();
    s.pointer_up(pt(80.0, 40.0), false, false);
    assert_eq!(stroke(&snapshot(&s)), 1.0);
    s.set_scale_stroke_width(false);
    drag(&mut s, pt(40.0, 20.0), pt(80.0, 40.0), false, false);
    assert_eq!(stroke(&snapshot(&s)), 1.0);
}

// ---------------------------------------------------------------------
// AC 29: toggling writes nothing
// ---------------------------------------------------------------------

#[test]
fn ac29_toggling_writes_nothing_and_save_bytes_are_identical() {
    for kind in ALL_KINDS {
        let mut s = selected(kind, Some(1.0));
        let from = vv(&s);
        let off_a = s.pack("0.1.0").unwrap();
        let off_b = s.pack("0.1.0").unwrap();
        assert_eq!(off_a, off_b, "baseline: packing is deterministic");
        s.set_scale_stroke_width(true);
        let on = s.pack("0.1.0").unwrap();
        assert_eq!(off_a, on, "{kind:?}: bytes differ with the switch on");
        s.set_scale_stroke_width(false);
        s.set_scale_stroke_width(true);
        s.set_scale_stroke_width(true);
        assert_eq!(off_a, s.pack("0.1.0").unwrap());
        assert_eq!(ops_since(&s, &from), ops_since(&s, &vv(&s)));
        let l = loro_of(&s);
        assert_eq!(l.oplog_vv(), from, "no operation recorded by toggling");
    }
    // Same for an empty document and with the Select tool deselected.
    let mut s = Session::new(1);
    let _warm_up = s.pack("0.1.0").unwrap(); // first pack of a new session differs
    let off = s.pack("0.1.0").unwrap();
    s.set_scale_stroke_width(true);
    assert_eq!(off, s.pack("0.1.0").unwrap());
}

#[test]
fn ac29_toggling_does_not_dirty_the_undo_history() {
    let mut s = selected(Kind::Rect, Some(1.0));
    let before = format!("{:?}", snapshot(&s));
    s.set_scale_stroke_width(true);
    s.set_scale_stroke_width(false);
    assert_eq!(before, format!("{:?}", snapshot(&s)));
}

// ---------------------------------------------------------------------
// AC 30 (engine side): the switch survives a tool change
// ---------------------------------------------------------------------

#[test]
fn ac30_the_state_is_kept_while_another_tool_is_active() {
    let mut s = Session::new(1);
    s.set_scale_stroke_width(true);
    for tool in [Tool::Node, Tool::Select] {
        s.set_tool(tool);
        assert!(s.scale_stroke_width(), "{tool:?}");
    }
    s.set_tool(Tool::Select);
    s.set_scale_stroke_width(false);
    s.set_tool(Tool::Node);
    s.set_tool(Tool::Select);
    assert!(!s.scale_stroke_width());
}

// ---------------------------------------------------------------------
// AC 9 / 31: the corner radius is independent of the switch
// ---------------------------------------------------------------------

fn rect_with_radius(radius: f64, stroke_mm: f64) -> Session {
    let d = Document::new(1);
    let id = d.create_rect(rb(0.0, 0.0, 40.0, 20.0));
    d.resize_rect(
        id,
        rb(0.0, 0.0, 40.0, 20.0),
        curvyo_document_core::CornerRadii::uniform(Length::from_mm(radius)),
        Some(Length::from_mm(stroke_mm)),
    )
    .unwrap();
    let mut s = open(&d);
    click(&mut s, pt(20.0, 0.0));
    s
}

fn radius_and_stroke(s: &Session) -> (f64, f64) {
    let ObjectSnapshot::Primitive(p) = snapshot(s) else {
        panic!()
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!()
    };
    (uniform_mm(corner_radii), p.style.stroke.width.as_mm())
}

#[test]
fn ac31_the_corner_radius_scales_identically_with_the_switch_on_and_off() {
    for (to, factor) in [
        (pt(60.0, 30.0), 1.5),                    // equal
        (pt(60.0, 50.0), (1.5_f64 * 2.5).sqrt()), // different
        (pt(80.0, 40.0), 2.0),                    // equal, larger
        (pt(2.0, 1.0), (0.05_f64 * 0.05).sqrt()), // shrink
    ] {
        let mut radii = vec![];
        for on in [false, true] {
            let mut s = rect_with_radius(2.0, 1.0);
            s.set_scale_corner_radius(true); // off by default since `unified-object-editing`
            s.set_scale_stroke_width(on);
            drag(&mut s, pt(40.0, 20.0), to, false, false);
            let (r, w) = radius_and_stroke(&s);
            let ObjectSnapshot::Primitive(p) = snapshot(&s) else {
                panic!()
            };
            let Shape::Rect { bounds, .. } = p.shape else {
                panic!()
            };
            let cap = bounds.width.as_mm().min(bounds.height.as_mm()) / 2.0;
            assert!(close(r, (2.0 * factor).min(cap)), "on {on}: radius {r}");
            if !on {
                assert_eq!(w, 1.0);
            }
            radii.push(r);
        }
        assert_eq!(radii[0], radii[1], "radius differs between modes");
    }
}

#[test]
fn ac31_every_handle_scales_the_radius_the_same_in_both_modes() {
    for hi in 0..8 {
        let mut radii = vec![];
        for on in [false, true] {
            let mut s = rect_with_radius(2.0, 1.0);
            s.set_scale_corner_radius(true);
            s.set_scale_stroke_width(on);
            let h = handles(Kind::Rect)[hi];
            let (press, release) = gesture(Kind::Rect, h, (1.5, 2.0), false);
            drag(&mut s, press, release, false, false);
            radii.push(radius_and_stroke(&s).0);
        }
        assert_eq!(radii[0], radii[1], "handle {hi}");
        assert!(radii[0] > 0.0);
    }
}

// ---------------------------------------------------------------------
// Peers: a concurrent edit survives a resize with the switch off
// ---------------------------------------------------------------------

fn merged(a: &Document, b: &Document) -> Document {
    let loro = loro::LoroDoc::new();
    loro.import(&a.export_loro_snapshot().unwrap()).unwrap();
    loro.import(&b.export_loro_snapshot().unwrap()).unwrap();
    loro.commit();
    let loro_bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": CURRENT_LORO_SNAPSHOT_VERSION,
        "app_version": "tester-merge",
    });
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    w.start_file("manifest.json", o).unwrap();
    w.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    w.start_file("document.loro", o).unwrap();
    w.write_all(&loro_bytes).unwrap();
    w.start_file("document.json", o).unwrap();
    w.write_all(b"{}").unwrap();
    unpack(9, &w.finish().unwrap().into_inner()).expect("merged opens")
}

#[test]
fn a_peers_stroke_edit_survives_a_resize_with_the_switch_off_in_both_merge_orders() {
    for kind in [Kind::Rect, Kind::Ellipse, Kind::Polygon, Kind::OpenPath] {
        for flip in [false, true] {
            let (base, id) = build(kind, None);
            let base_bytes = pack(&base, "0.1.0").unwrap();
            // Peer 2: the session resizing with the switch off.
            let mut s = Session::open(2, &base_bytes).unwrap();
            s.set_tool(Tool::Select);
            let p = select_point(kind, &s);
            click(&mut s, p);
            let h = handles(kind)[0];
            let (press, release) = gesture(kind, h, (1.5, 1.5), false);
            drag(&mut s, press, release, false, false);
            // Peer 3: concurrently sets the stroke to 2.0 (and, for a rect,
            // writes nothing else that matters).
            let peer = unpack(3, &base_bytes).unwrap();
            match kind {
                Kind::Rect => peer
                    .resize_rect(
                        id,
                        rb(0.0, 0.0, 40.0, 20.0),
                        curvyo_document_core::CornerRadii::uniform(Length::from_mm(0.0)),
                        Some(Length::from_mm(2.0)),
                    )
                    .unwrap(),
                Kind::Ellipse => peer
                    .resize_ellipse(
                        id,
                        EllipseFrame {
                            center: pt(20.0, 10.0),
                            rx: Length::from_mm(20.0),
                            ry: Length::from_mm(10.0),
                        },
                        Some(Length::from_mm(2.0)),
                    )
                    .unwrap(),
                Kind::Polygon => peer
                    .resize_star_frame(
                        id,
                        StarFrame {
                            center: pt(30.0, 30.0),
                            radius: Length::from_mm(10.0),
                            angle: Angle::from_radians(-FRAC_PI_2),
                        },
                        Some(Length::from_mm(2.0)),
                    )
                    .unwrap(),
                _ => peer
                    .resize_path(id, &[], Some(Length::from_mm(2.0)))
                    .unwrap(),
            }
            let mine = unpack(4, &s.pack("0.1.0").unwrap()).unwrap();
            let m = if flip {
                merged(&peer, &mine)
            } else {
                merged(&mine, &peer)
            };
            let w = stroke(&m.object(id).unwrap());
            assert_eq!(w, 2.0, "{kind:?} flip {flip}: peer stroke lost, got {w}");
        }
    }
}

#[test]
fn an_equal_value_corner_radius_is_not_rewritten_so_a_peers_radius_survives() {
    for flip in [false, true] {
        let d = Document::new(1);
        let id = d.create_rect(rb(0.0, 0.0, 40.0, 20.0));
        let base_bytes = pack(&d, "0.1.0").unwrap();
        let mut s = Session::open(2, &base_bytes).unwrap();
        s.set_tool(Tool::Select);
        click(&mut s, pt(20.0, 0.0));
        s.pointer_hover(pt(40.0, 20.0), false, false);
        drag(&mut s, pt(40.0, 20.0), pt(60.0, 30.0), false, false);
        let from = vv(&s);
        let _ = from;
        let peer = unpack(3, &base_bytes).unwrap();
        peer.set_corner_radius(&[id], Length::from_mm(3.0)).unwrap();
        let mine = unpack(4, &s.pack("0.1.0").unwrap()).unwrap();
        let m = if flip {
            merged(&peer, &mine)
        } else {
            merged(&mine, &peer)
        };
        let ObjectSnapshot::Primitive(p) = m.object(id).unwrap() else {
            panic!()
        };
        let Shape::Rect {
            corner_radii,
            bounds,
        } = p.shape
        else {
            panic!()
        };
        assert_eq!(
            uniform_mm(corner_radii),
            3.0,
            "flip {flip}: peer radius lost"
        );
        assert!(close(bounds.width.as_mm(), 60.0), "resize survived");
    }
}

#[test]
fn a_resize_of_a_zero_radius_rect_writes_no_corner_radius_key() {
    for on in [false, true] {
        let d = Document::new(1);
        let _ = d.create_rect(rb(0.0, 0.0, 40.0, 20.0));
        let mut s = open(&d);
        click(&mut s, pt(20.0, 0.0));
        s.set_scale_stroke_width(on);
        let from = vv(&s);
        drag(&mut s, pt(40.0, 20.0), pt(60.0, 30.0), false, false);
        let ops = ops_since(&s, &from);
        assert!(!ops.contains("corner_radius"), "on {on}: {ops}");
    }
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
