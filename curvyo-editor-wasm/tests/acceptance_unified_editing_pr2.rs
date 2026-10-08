//! Independent tester acceptance tests for PR 2 of
//! `specs/0009-unified-object-editing/specification.md` (criteria 25-34: the shape
//! tools only create, double-click) plus a regression sweep of the flows that
//! cross the change (draw with a shape tool, then edit through the Select
//! tool; pen and node untouched). Written from the specification before the
//! implementation diff was read. Everything goes through `Session`'s public
//! API; the browser host's double-click sequence is simulated the way the
//! older tester suites do (press and release, then one `double_click`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::similar_names, clippy::doc_markdown)]
#![allow(clippy::many_single_char_names, clippy::cast_precision_loss)]
#![allow(
    missing_docs,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments
)]

use std::collections::HashSet;
use std::f64::consts::SQRT_2;

use curvyo_document_core::{
    AnchorId, Document, EllipseFrame, Length, NewAnchor, Point, PointCount, RectBounds, Shape,
    outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::PolyStarMode;

const CREATION: [Tool; 3] = [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar];

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

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn open_session(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).expect("session output must reopen")
}

fn bytes(s: &Session) -> Vec<u8> {
    s.pack("0.1.0").unwrap()
}

fn change_count(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.len_changes()
}

fn n_objects(s: &Session) -> usize {
    doc_of(s).object_ids().len()
}

fn prim(s: &Session, index: usize) -> curvyo_document_core::PrimitiveSnapshot {
    let d = doc_of(s);
    d.primitive(d.object_ids()[index]).unwrap()
}

fn shape_dbg(s: &Session, index: usize) -> String {
    let p = prim(s, index);
    format!("{:?} rot={:?}", p.shape, p.rotation)
}

fn k_of(s: &Session) -> f64 {
    s.view().scale()
}

fn click(s: &mut Session, p: Point, shift: bool) {
    s.pointer_hover(p, shift, false);
    s.pointer_down(p, shift);
    s.pointer_up(p, shift, false);
}

fn drag(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(from, shift, ctrl);
    s.pointer_down(from, shift);
    s.pointer_hover(to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
}

/// The browser host's double click: first press and release reach the
/// session, the second press is withheld and its release becomes one
/// `double_click`, followed by a hover.
fn dbl(s: &mut Session, first: Point, second: Point) -> bool {
    s.pointer_hover(first, false, false);
    s.pointer_down(first, false);
    s.pointer_up(first, false, false);
    s.pointer_hover(second, false, false);
    let hint = s.double_click(second, false, false);
    s.pointer_hover(second, false, false);
    hint
}

/// Box of a primitive in document space (axis-aligned, rotation ignored).
fn box_of(p: &curvyo_document_core::PrimitiveSnapshot) -> (f64, f64, f64, f64) {
    let pts = outline_of_rotated(&p.shape, p.rotation);
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for a in &pts {
        x0 = x0.min(a.point.x);
        y0 = y0.min(a.point.y);
        x1 = x1.max(a.point.x);
        y1 = y1.max(a.point.y);
    }
    (x0, y0, x1, y1)
}

fn outline_point(s: &Session, index: usize) -> Point {
    let p = prim(s, index);
    outline_of_rotated(&p.shape, p.rotation)[0].point
}

/// Decoration vertices: every vertex of `s`'s draw list that a copy with
/// nothing selected, in the same tool, does not have.
fn decoration(s: &Session, tool: Tool) -> Vec<Point> {
    let mut copy = Session::open(1, &bytes(s)).unwrap();
    copy.set_tool(tool);
    let base: HashSet<String> = copy
        .draw_list()
        .triangles
        .iter()
        .map(|v| format!("{v:?}"))
        .collect();
    s.draw_list()
        .triangles
        .iter()
        .filter(|v| !base.contains(&format!("{v:?}")))
        .map(|v| v.position)
        .collect()
}

fn extent(points: &[Point]) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    (x0, y0, x1, y1)
}

/// How far (in px) the decoration reaches beyond `b` in any direction.
fn overshoot_px(dec: &[Point], b: (f64, f64, f64, f64), k: f64) -> f64 {
    let e = extent(dec);
    [b.0 - e.0, b.1 - e.1, e.2 - b.2, e.3 - b.3]
        .into_iter()
        .fold(f64::MIN, f64::max)
        * k
}

/// A box in px at the session's scale around centre `c` (mm), turned by `th`.
#[derive(Clone, Copy)]
struct Fr {
    c: Point,
    hw: f64,
    hh: f64,
    th: f64,
    k: f64,
}

impl Fr {
    fn at(&self, ox: f64, oy: f64) -> Point {
        rot(
            pt(self.c.x + ox / self.k, self.c.y + oy / self.k),
            self.c,
            self.th,
        )
    }
    fn w(&self) -> f64 {
        self.hw * 2.0 * self.k
    }
    fn h(&self) -> f64 {
        self.hh * 2.0 * self.k
    }
    fn corner(&self, sx: f64, sy: f64) -> Point {
        self.at(sx * self.w() / 2.0, sy * self.h() / 2.0)
    }
    fn mid(&self, nx: f64, ny: f64) -> Point {
        self.at(nx * self.w() / 2.0, ny * self.h() / 2.0)
    }
    fn rot_corner(&self, sx: f64, sy: f64) -> Point {
        let d = 32.0 / SQRT_2;
        self.at(sx * (self.w() / 2.0 + d), sy * (self.h() / 2.0 + d))
    }
    /// The radius handle at radius 0 on corner (sx, sy): 15 px along the
    /// inward diagonal.
    fn radius0(&self, sx: f64, sy: f64) -> Point {
        let (bx, by) = (sx * self.w() / 2.0, sy * self.h() / 2.0);
        self.at(bx - sx * 15.0 / SQRT_2, by - sy * 15.0 / SQRT_2)
    }
}

/// An unrotated rectangle of `w` x `h` px at the default zoom, centre (60, 45)
/// mm, selected through the Select tool.
fn rect_scene(w: f64, h: f64, th: f64) -> (Session, Fr) {
    let k = k_of(&Session::new(1));
    let d = Document::new(1);
    let (wm, hm) = (w / k, h / k);
    let _ = d.create_rect(bounds(60.0 - wm / 2.0, 45.0 - hm / 2.0, wm, hm));
    if th != 0.0 {
        let id = d.object_ids()[0];
        let o = d.object(id).unwrap();
        d.rotate_object(&o.rotated(
            pt(60.0, 45.0),
            curvyo_document_core::Angle::from_radians(th),
        ))
        .unwrap();
    }
    let mut s = open_session(&d);
    let fr = Fr {
        c: pt(60.0, 45.0),
        hw: wm / 2.0,
        hh: hm / 2.0,
        th,
        k,
    };
    let p = outline_point(&s, 0);
    click(&mut s, p, false);
    (s, fr)
}

/// The shape that a fresh session creates for `from` to `to` with `tool`.
fn reference_shape(tool: Tool, from: Point, to: Point, shift: bool, ctrl: bool) -> String {
    let mut s = Session::new(7);
    s.set_tool(tool);
    drag(&mut s, from, to, shift, ctrl);
    assert_eq!(n_objects(&s), 1, "{tool:?}: reference drag creates one");
    shape_dbg(&s, 0)
}

/// Deletes the selection with the Select tool and returns how many objects
/// remain, then a description of the survivors.
fn delete_selection(s: &mut Session) -> Vec<String> {
    s.set_tool(Tool::Select);
    s.delete_selected();
    let n = n_objects(s);
    (0..n).map(|i| shape_dbg(s, i)).collect()
}

// =====================================================================
// Criterion 25: a press in a creation tool always creates
// =====================================================================

#[test]
fn ac25_a_drag_from_anywhere_on_or_in_an_existing_shape_creates_a_new_shape_and_touches_nothing_else()
 {
    for th in [0.0, 0.5] {
        for tool in CREATION {
            for shift in [false, true] {
                // Spots (offsets from the existing box in px).
                let spot_names = [
                    "outline",
                    "inside the box",
                    "centre handle",
                    "corner resize handle",
                    "edge resize handle",
                    "corner rotate handle",
                    "radius handle",
                ];
                for (i, name) in spot_names.iter().enumerate() {
                    let (mut s, fr) = rect_scene(200.0, 140.0, th);
                    let before = shape_dbg(&s, 0);
                    let from = match i {
                        0 => fr.at(-50.0, -70.0),
                        1 => fr.at(30.0, 20.0),
                        2 => fr.at(0.0, 0.0),
                        3 => fr.corner(1.0, 1.0),
                        4 => fr.mid(1.0, 0.0),
                        5 => fr.rot_corner(-1.0, -1.0),
                        _ => fr.radius0(1.0, -1.0),
                    };
                    let to = pt(from.x + 150.0 / fr.k, from.y + 110.0 / fr.k);
                    s.set_tool(tool);
                    drag(&mut s, from, to, shift, false);
                    let label = format!("{tool:?} shift={shift} th={th} at {name}");
                    assert_eq!(n_objects(&s), 2, "{label}: exactly one new object");
                    assert_eq!(shape_dbg(&s, 0), before, "{label}: old shape untouched");
                    assert_eq!(
                        shape_dbg(&s, 1),
                        reference_shape(tool, from, to, shift, false),
                        "{label}: created exactly as on empty canvas"
                    );
                    assert_eq!(s.tool(), Tool::Select, "{label}: Select active after");
                    let survivors = delete_selection(&mut s);
                    assert_eq!(
                        survivors,
                        vec![before.clone()],
                        "{label}: the new shape is the sole selection"
                    );
                }
            }
        }
    }
}

#[test]
fn ac25_creation_drags_follow_the_constraint_and_create_exactly_as_0003() {
    let k = k_of(&Session::new(1));
    let a = pt(30.0, 30.0);
    let b = pt(30.0 + 90.0 / k, 30.0 + 40.0 / k);
    // Rectangle: opposite corners; Ctrl: a square of the larger extent.
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(&mut s, a, b, false, false);
    match prim(&s, 0).shape {
        Shape::Rect {
            bounds: r,
            corner_radii,
        } => {
            assert!(near(r.origin.x, a.x, 1e-9) && near(r.origin.y, a.y, 1e-9));
            assert!(near(r.width.as_mm(), 90.0 / k, 1e-9));
            assert!(near(r.height.as_mm(), 40.0 / k, 1e-9));
            assert!(uniform_mm(corner_radii).abs() < 1e-12);
        }
        other => panic!("rect expected {other:?}"),
    }
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(&mut s, a, b, false, true);
    match prim(&s, 0).shape {
        Shape::Rect { bounds: r, .. } => {
            assert!(near(r.width.as_mm(), r.height.as_mm(), 1e-9));
            assert!(near(r.width.as_mm(), 90.0 / k, 1e-9));
        }
        other => panic!("rect expected {other:?}"),
    }
    // Ellipse: box corners; Ctrl: circle.
    let mut s = Session::new(1);
    s.set_tool(Tool::Ellipse);
    drag(&mut s, a, b, false, false);
    match prim(&s, 0).shape {
        Shape::Ellipse { frame } => {
            assert!(near(frame.rx.as_mm(), 45.0 / k, 1e-9));
            assert!(near(frame.ry.as_mm(), 20.0 / k, 1e-9));
            assert!(near(frame.center.x, a.x + 45.0 / k, 1e-9));
        }
        other => panic!("ellipse expected {other:?}"),
    }
    let mut s = Session::new(1);
    s.set_tool(Tool::Ellipse);
    drag(&mut s, a, b, false, true);
    match prim(&s, 0).shape {
        Shape::Ellipse { frame } => assert!(near(frame.rx.as_mm(), frame.ry.as_mm(), 1e-9)),
        other => panic!("ellipse expected {other:?}"),
    }
    // Polygon and star: centre A, one vertex at B.
    let mut s = Session::new(1);
    s.set_tool(Tool::PolygonStar);
    s.set_poly_star_mode(PolyStarMode::Polygon);
    s.set_poly_star_point_count(PointCount::new(6).unwrap());
    drag(&mut s, a, b, false, false);
    match prim(&s, 0).shape {
        Shape::Polygon {
            frame,
            point_count: count,
        } => {
            assert_eq!(count.get(), 6);
            assert!(near(frame.center.x, a.x, 1e-9) && near(frame.center.y, a.y, 1e-9));
            let rb = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            assert!(near(frame.radius.as_mm(), rb, 1e-9));
        }
        other => panic!("polygon expected {other:?}"),
    }
    s.set_tool(Tool::PolygonStar);
    s.set_poly_star_mode(PolyStarMode::Star);
    drag(&mut s, pt(200.0, 200.0), pt(230.0, 200.0), false, false);
    assert!(matches!(prim(&s, 1).shape, Shape::Star { .. }));
}

#[test]
fn ac25_no_hover_highlight_cursor_state_or_hint_over_existing_objects_in_a_creation_tool() {
    let k = k_of(&Session::new(1));
    let d = Document::new(1);
    let _ = d.create_rect(bounds(20.0, 20.0, 200.0 / k, 140.0 / k));
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(120.0, 60.0),
        rx: Length::from_mm(60.0 / k),
        ry: Length::from_mm(40.0 / k),
    });
    let mut s = open_session(&d);
    // Select the rectangle, leave the ellipse unselected.
    let p = outline_point(&s, 0);
    click(&mut s, p, false);
    let fr = Fr {
        c: pt(20.0 + 100.0 / k, 20.0 + 70.0 / k),
        hw: 100.0 / k,
        hh: 70.0 / k,
        th: 0.0,
        k,
    };
    for tool in CREATION {
        s.set_tool(tool);
        let far = pt(900.0, 900.0);
        s.pointer_hover(far, false, false);
        let reference = s.draw_list();
        let (cur0, hint0) = (s.cursor_hint(), s.handle_hint());
        assert!(hint0.is_empty(), "{tool:?}: handle hint far away");
        let ellipse_outline = outline_point(&s, 1);
        for (name, p) in [
            ("selected outline", fr.at(-30.0, -70.0)),
            ("selected corner handle", fr.corner(1.0, 1.0)),
            ("selected rotate handle", fr.rot_corner(1.0, -1.0)),
            ("selected centre", fr.at(0.0, 0.0)),
            ("unselected outline", ellipse_outline),
        ] {
            for shift in [false, true] {
                s.pointer_hover(p, shift, false);
                assert_eq!(
                    s.draw_list(),
                    reference,
                    "{tool:?} {name} shift={shift}: no hover highlight and no handles"
                );
                assert_eq!(s.cursor_hint(), cur0, "{tool:?} {name}: cursor hint");
                assert_eq!(s.handle_hint(), hint0, "{tool:?} {name}: handle hint");
            }
        }
    }
}

// =====================================================================
// Criterion 26: a press without movement writes nothing, keeps the selection
// =====================================================================

#[test]
fn ac26_a_press_without_movement_creates_nothing_and_leaves_selection_empty_and_tool_alone() {
    let k = k_of(&Session::new(1));
    for tool in CREATION {
        for shift in [false, true] {
            for spot in 0..6 {
                let d = Document::new(1);
                let _ = d.create_rect(bounds(20.0, 20.0, 200.0 / k, 140.0 / k)); // A
                let _ = d.create_rect(bounds(120.0, 120.0, 100.0 / k, 80.0 / k)); // B
                let mut s = open_session(&d);
                let a_outline = outline_point(&s, 0);
                click(&mut s, a_outline, false); // A selected
                let fr = Fr {
                    c: pt(20.0 + 100.0 / k, 20.0 + 70.0 / k),
                    hw: 100.0 / k,
                    hh: 70.0 / k,
                    th: 0.0,
                    k,
                };
                s.set_tool(tool);
                let far = pt(900.0, 900.0);
                s.pointer_hover(far, false, false);
                let draw_before = s.draw_list();
                let bytes_before = bytes(&s);
                let changes_before = change_count(&s);
                let p = match spot {
                    0 => far,                    // empty canvas
                    1 => outline_point(&s, 1),   // unselected object's outline
                    2 => fr.at(-30.0, -70.0),    // selected outline
                    3 => fr.corner(1.0, 1.0),    // handle spot
                    4 => fr.at(10.0, 10.0),      // inside the selected box
                    _ => fr.radius0(-1.0, -1.0), // radius handle spot
                };
                s.pointer_hover(p, shift, false);
                s.pointer_down(p, shift);
                s.pointer_up(p, shift, false);
                s.pointer_hover(far, false, false);
                let label = format!("{tool:?} shift={shift} spot {spot}");
                assert_eq!(s.tool(), tool, "{label}: tool unchanged");
                assert_eq!(change_count(&s), changes_before, "{label}: no commit");
                assert_eq!(bytes(&s), bytes_before, "{label}: nothing written");
                assert_eq!(s.draw_list(), draw_before, "{label}: selection unchanged");
                // Choosing the creation tool cleared the selection (customer
                // decision 2026-10-06), and the press did not select anything:
                // deleting the selection removes nothing.
                assert!(decoration(&s, tool).is_empty(), "{label}: no box drawn");
                let survivors = delete_selection(&mut s);
                assert_eq!(
                    survivors.len(),
                    2,
                    "{label}: nothing selected, nothing gone"
                );
            }
        }
    }
}

#[test]
fn ac26_a_press_without_movement_with_nothing_selected_keeps_it_that_way() {
    for tool in CREATION {
        let k = k_of(&Session::new(1));
        let d = Document::new(1);
        let _ = d.create_rect(bounds(20.0, 20.0, 200.0 / k, 140.0 / k));
        let mut s = open_session(&d);
        s.set_tool(tool);
        let p = outline_point(&s, 0);
        click(&mut s, p, false); // must not select
        s.pointer_hover(pt(900.0, 900.0), false, false);
        assert!(
            decoration(&s, tool).is_empty(),
            "{tool:?}: a click on an outline must not select"
        );
        assert_eq!(
            delete_selection(&mut s).len(),
            1,
            "{tool:?}: nothing deleted"
        );
    }
}

// =====================================================================
// Criterion 27 (reversed by the customer on 2026-10-06): choosing a creation
// tool clears the selection and no selection box is drawn under it
// =====================================================================

#[test]
fn ac27_choosing_a_creation_tool_clears_the_selection_and_draws_no_box() {
    let k = k_of(&Session::new(1));
    // One rectangle, one star and one path, so the rule is tried on every kind.
    let d = Document::new(1);
    let _ = d.create_rect(bounds(20.0, 20.0, 200.0 / k, 140.0 / k));
    let _ = d.create_star(
        curvyo_document_core::StarFrame {
            center: pt(150.0, 120.0),
            radius: Length::from_mm(90.0 / k),
            angle: curvyo_document_core::Angle::from_radians(-std::f64::consts::FRAC_PI_2),
        },
        PointCount::new(8).unwrap(),
        curvyo_document_core::InnerRatio::new(0.5).unwrap(),
    );
    let _ = d.create_path(
        &[
            anchor(1, 400.0, 30.0),
            anchor(2, 400.0 + 160.0 / k, 30.0),
            anchor(3, 400.0 + 160.0 / k, 30.0 + 120.0 / k),
        ],
        true,
    );
    // 0 = rect, 1 = star, 2 = rect and star (shift), 3 = path.
    let selected = |which: usize| {
        let mut s = open_session(&d);
        let (first, second) = (outline_point(&s, 0), outline_point(&s, 1));
        match which {
            0 => click(&mut s, first, false),
            1 => click(&mut s, second, false),
            2 => {
                click(&mut s, first, false);
                click(&mut s, second, true);
            }
            _ => click(&mut s, pt(400.0 + 80.0 / k, 30.0), false),
        }
        s.pointer_hover(pt(900.0, 900.0), false, false);
        s
    };
    for which in 0..4 {
        let s = selected(which);
        assert!(
            !decoration(&s, Tool::Select).is_empty(),
            "which {which}: the Select tool draws the selection"
        );
        for tool in CREATION {
            let mut c = selected(which);
            c.set_tool(tool);
            c.pointer_hover(pt(900.0, 900.0), false, false);
            assert!(
                decoration(&c, tool).is_empty(),
                "{tool:?} which {which}: no selection box under a creation tool"
            );
            c.set_tool(Tool::Select);
            c.delete_selected();
            assert_eq!(
                n_objects(&c),
                n_objects(&s),
                "{tool:?} which {which}: the selection was cleared, nothing is deleted"
            );
        }
    }
}

#[test]
fn ac27_the_node_and_pen_tools_keep_the_selection() {
    let k = k_of(&Session::new(1));
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 40.0, 30.0),
            anchor(2, 40.0 + 160.0 / k, 30.0),
            anchor(3, 40.0 + 160.0 / k, 30.0 + 120.0 / k),
        ],
        true,
    );
    for tool in [Tool::Node, Tool::Pen] {
        let mut s = open_session(&d);
        click(&mut s, pt(40.0 + 80.0 / k, 30.0), false);
        s.set_tool(tool);
        s.set_tool(Tool::Select);
        s.delete_selected();
        assert_eq!(n_objects(&s), 0, "{tool:?}: the path was still selected");
    }
}

// =====================================================================
// Criterion 28: a create-drag hands over to the Select tool
// =====================================================================

#[test]
fn ac28_a_create_drag_activates_select_with_the_new_shape_alone_selected_and_handles_drawn() {
    let k = k_of(&Session::new(1));
    for tool in CREATION {
        for ctrl in [false, true] {
            let d = Document::new(1);
            let _ = d.create_rect(bounds(300.0, 300.0, 50.0 / k, 50.0 / k));
            let mut s = open_session(&d);
            let p = outline_point(&s, 0);
            click(&mut s, p, false); // a previous selection that must be replaced
            s.set_tool(tool);
            drag(
                &mut s,
                pt(20.0, 20.0),
                pt(20.0 + 200.0 / k, 20.0 + 140.0 / k),
                false,
                ctrl,
            );
            assert_eq!(s.tool(), Tool::Select, "{tool:?} ctrl={ctrl}");
            assert_eq!(n_objects(&s), 2);
            s.pointer_hover(pt(900.0, 900.0), false, false);
            let b = box_of(&prim(&s, 1));
            let dec = decoration(&s, Tool::Select);
            assert!(
                overshoot_px(&dec, b, k) > 25.0,
                "{tool:?} ctrl={ctrl}: the full handle set is drawn at once"
            );
            // Only the new shape is selected: the old one has no decoration
            // (its box would reach outside the new box's neighbourhood).
            let old = box_of(&prim(&s, 0));
            let near_old = dec.iter().any(|v| v.x >= old.0 - 1.0 && v.y >= old.1 - 1.0);
            assert!(!near_old, "{tool:?} ctrl={ctrl}: old object is deselected");
            let survivors = delete_selection(&mut s);
            assert_eq!(survivors.len(), 1, "{tool:?}: sole selection deleted");
        }
    }
}

#[test]
fn ac28_the_new_shape_can_be_edited_with_its_handles_straight_away() {
    let k = k_of(&Session::new(1));
    // Rectangle: drag the new shape's corner resize handle.
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(
        &mut s,
        pt(20.0, 20.0),
        pt(20.0 + 200.0 / k, 20.0 + 140.0 / k),
        false,
        false,
    );
    let corner = pt(20.0 + 200.0 / k, 20.0 + 140.0 / k);
    drag(
        &mut s,
        corner,
        pt(corner.x + 40.0 / k, corner.y + 20.0 / k),
        false,
        false,
    );
    match prim(&s, 0).shape {
        Shape::Rect { bounds: r, .. } => {
            assert!(near(r.width.as_mm(), 240.0 / k, 1e-6), "{r:?}");
            assert!(near(r.height.as_mm(), 160.0 / k, 1e-6), "{r:?}");
            assert!(near(r.origin.x, 20.0, 1e-6) && near(r.origin.y, 20.0, 1e-6));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(n_objects(&s), 1, "the drag edited, it did not create");
    // Star: drag the inner-radius handle (first inner vertex) of a new star.
    let mut s = Session::new(1);
    s.set_tool(Tool::PolygonStar);
    s.set_poly_star_mode(PolyStarMode::Star);
    s.set_poly_star_point_count(PointCount::new(8).unwrap());
    s.set_poly_star_ratio(curvyo_document_core::InnerRatio::new(0.5).unwrap());
    drag(
        &mut s,
        pt(100.0, 100.0),
        pt(100.0, 100.0 + 100.0 / k),
        false,
        false,
    );
    assert_eq!(s.tool(), Tool::Select);
    let p = prim(&s, 0);
    let pts = outline_of_rotated(&p.shape, p.rotation);
    let inner = pts[1].point;
    let c = pt(100.0, 100.0);
    let dir = (inner.x - c.x, inner.y - c.y);
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
    let target = pt(
        c.x + dir.0 / len * 0.8 * 100.0 / k,
        c.y + dir.1 / len * 0.8 * 100.0 / k,
    );
    drag(&mut s, inner, target, false, false);
    match prim(&s, 0).shape {
        Shape::Star {
            inner_ratio, frame, ..
        } => {
            assert!(near(frame.radius.as_mm(), 100.0 / k, 1e-6), "outer fixed");
            assert!(
                near(inner_ratio.get(), 0.8, 1e-6),
                "ratio {}",
                inner_ratio.get()
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn ac28_escape_and_degenerate_drags_leave_the_creation_tool_active() {
    for tool in CREATION {
        let mut s = Session::new(1);
        s.set_tool(tool);
        s.pointer_down(pt(10.0, 10.0), false);
        s.pointer_hover(pt(60.0, 60.0), false, false);
        s.escape();
        s.pointer_up(pt(60.0, 60.0), false, false);
        assert_eq!(n_objects(&s), 0, "{tool:?}: Escape cancels the create-drag");
        assert_eq!(s.tool(), tool, "{tool:?}: tool unchanged after Escape");
        drag(&mut s, pt(10.0, 10.0), pt(10.0, 10.0), false, false);
        assert_eq!(n_objects(&s), 0);
        assert_eq!(s.tool(), tool);
    }
}

#[test]
fn ac28_two_create_drags_in_a_row_with_the_tool_set_again_make_two_shapes() {
    let k = k_of(&Session::new(1));
    let mut s = Session::new(1);
    for i in 0..3 {
        s.set_tool(Tool::Rectangle);
        let x = 20.0 + f64::from(i) * 100.0;
        drag(
            &mut s,
            pt(x, 20.0),
            pt(x + 60.0 / k, 20.0 + 40.0 / k),
            false,
            false,
        );
        assert_eq!(s.tool(), Tool::Select);
    }
    assert_eq!(n_objects(&s), 3);
    // Only the third is selected.
    let survivors = delete_selection(&mut s);
    assert_eq!(survivors.len(), 2);
}

// =====================================================================
// Criterion 29: next-shape settings
// =====================================================================

#[test]
fn ac29_next_shape_settings_persist_and_never_change_a_selected_shape() {
    let k = k_of(&Session::new(1));
    let mut s = Session::new(1);
    s.set_tool(Tool::PolygonStar);
    s.set_poly_star_mode(PolyStarMode::Star);
    s.set_poly_star_point_count(PointCount::new(7).unwrap());
    s.set_poly_star_ratio(curvyo_document_core::InnerRatio::new(0.3).unwrap());
    drag(
        &mut s,
        pt(80.0, 80.0),
        pt(80.0 + 90.0 / k, 80.0),
        false,
        false,
    );
    let first = shape_dbg(&s, 0);
    assert!(
        first.contains("count: PointCount(7)") || first.contains('7'),
        "{first}"
    );
    // Settings persist for the next shape.
    s.set_tool(Tool::PolygonStar);
    assert_eq!(s.poly_star_point_count().get(), 7);
    assert!(near(s.poly_star_ratio().get(), 0.3, 1e-12));
    assert_eq!(s.poly_star_mode(), PolyStarMode::Star);
    drag(
        &mut s,
        pt(300.0, 80.0),
        pt(300.0 + 90.0 / k, 80.0),
        false,
        false,
    );
    match prim(&s, 1).shape {
        Shape::Star {
            point_count: count,
            inner_ratio,
            ..
        } => {
            assert_eq!(count.get(), 7);
            assert!(near(inner_ratio.get(), 0.3, 1e-12));
        }
        other => panic!("{other:?}"),
    }
    // Changing the next-shape settings with the second star selected (and the
    // creation tool active) changes neither star.
    s.set_tool(Tool::PolygonStar);
    let changes = change_count(&s);
    let (d0, d1) = (shape_dbg(&s, 0), shape_dbg(&s, 1));
    s.set_poly_star_point_count(PointCount::new(12).unwrap());
    s.set_poly_star_ratio(curvyo_document_core::InnerRatio::new(0.9).unwrap());
    s.set_poly_star_mode(PolyStarMode::Polygon);
    assert_eq!(change_count(&s), changes, "no commit");
    assert_eq!((shape_dbg(&s, 0), shape_dbg(&s, 1)), (d0, d1));
    // The next shape uses the new settings.
    drag(
        &mut s,
        pt(500.0, 80.0),
        pt(500.0 + 90.0 / k, 80.0),
        false,
        false,
    );
    match prim(&s, 2).shape {
        Shape::Polygon {
            point_count: count, ..
        } => assert_eq!(count.get(), 12),
        other => panic!("{other:?}"),
    }
}

// =====================================================================
// Criteria 31-34: double-click
// =====================================================================

fn triangle_path_doc(k: f64) -> Document {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 40.0, 30.0),
            anchor(2, 40.0 + 200.0 / k, 30.0),
            anchor(3, 40.0 + 200.0 / k, 30.0 + 140.0 / k),
        ],
        true,
    );
    d
}

#[test]
fn ac31_double_click_on_a_path_outline_centre_or_inside_hands_off_to_the_node_tool() {
    let k = k_of(&Session::new(1));
    let c = pt(40.0 + 100.0 / k, 30.0 + 70.0 / k);
    for (name, p, pre_select) in [
        ("outline (unselected)", pt(40.0 + 50.0 / k, 30.0), false),
        ("outline (selected)", pt(40.0 + 50.0 / k, 30.0), true),
        // The centre handle of a selected path opens the typed move since
        // `edit-interaction-polish` PR 3 (criterion 15); its tests are in
        // `edit_polish_typed_skew_move.rs`.
        (
            "inside the box (selected)",
            pt(c.x + 40.0 / k, c.y + 10.0 / k),
            true,
        ),
        ("hypotenuse", pt(40.0 + 100.0 / k, 30.0 + 70.0 / k), false),
    ] {
        let mut s = open_session(&triangle_path_doc(k));
        if pre_select {
            click(&mut s, pt(40.0 + 100.0 / k, 30.0), false);
        }
        let before = bytes(&s);
        let hint = dbl(&mut s, p, p);
        assert_eq!(s.tool(), Tool::Node, "{name}: hand-off");
        assert!(!hint, "{name}: no hint chip for a path");
        assert_eq!(bytes(&s), before, "{name}: nothing written");
        // The path is editable in the Node tool straight away: dragging its
        // first node moves it.
        let n0 = pt(40.0, 30.0);
        s.pointer_hover(n0, false, false);
        s.pointer_down(n0, false);
        s.pointer_hover(pt(45.0, 33.0), false, false);
        s.pointer_up(pt(45.0, 33.0), false, false);
        let d = doc_of(&s);
        let path = d.path(d.object_ids()[0]).unwrap();
        assert_eq!(path.anchors[0].point, pt(45.0, 33.0), "{name}: node moved");
        assert_eq!(s.tool(), Tool::Node);
    }
}

fn primitive_docs(k: f64) -> Vec<(&'static str, Document, Point)> {
    // (name, doc, an outline point). Box is 200 x 140 px or 160 x 160 for the
    // regular shapes, always >= 72 px so every handle is drawn.
    let mk = Document::new;
    let r = mk(1);
    let _ = r.create_rect(bounds(40.0, 30.0, 200.0 / k, 140.0 / k));
    let e = mk(1);
    let _ = e.create_ellipse(EllipseFrame {
        center: pt(40.0 + 100.0 / k, 30.0 + 70.0 / k),
        rx: Length::from_mm(100.0 / k),
        ry: Length::from_mm(70.0 / k),
    });
    let frame = curvyo_document_core::StarFrame {
        center: pt(40.0 + 100.0 / k, 30.0 + 100.0 / k),
        radius: Length::from_mm(80.0 / k),
        angle: curvyo_document_core::Angle::from_radians(-std::f64::consts::FRAC_PI_2),
    };
    let p = mk(1);
    let _ = p.create_polygon(frame, PointCount::new(8).unwrap());
    let st = mk(1);
    let _ = st.create_star(
        frame,
        PointCount::new(8).unwrap(),
        curvyo_document_core::InnerRatio::new(0.5).unwrap(),
    );
    // A plain outline point that is on no handle spot: a quarter along the
    // top edge of a rectangle, 45 degrees on an ellipse, the middle of the
    // first edge of a polygon or star.
    let rect_pt = pt(40.0 + 50.0 / k, 30.0);
    let ell_pt = pt(
        40.0 + 100.0 / k + 100.0 / k * std::f64::consts::FRAC_1_SQRT_2,
        30.0 + 70.0 / k + 70.0 / k * std::f64::consts::FRAC_1_SQRT_2,
    );
    let mid = |d: &Document| {
        let id = d.object_ids()[0];
        let o = d.primitive(id).unwrap();
        let pts = outline_of_rotated(&o.shape, o.rotation);
        pt(
            f64::midpoint(pts[0].point.x, pts[1].point.x),
            f64::midpoint(pts[0].point.y, pts[1].point.y),
        )
    };
    let (po, so) = (mid(&p), mid(&st));
    vec![
        ("rectangle", r, rect_pt),
        ("ellipse", e, ell_pt),
        ("polygon", p, po),
        ("star", st, so),
    ]
}

#[test]
fn ac32_double_click_on_a_primitive_changes_nothing_and_asks_for_the_hint() {
    let k = k_of(&Session::new(1));
    for (name, d, outline) in primitive_docs(k) {
        let o = open_session(&d);
        let b = box_of(&prim(&o, 0));
        let centre = pt(f64::midpoint(b.0, b.2), f64::midpoint(b.1, b.3));
        // Not on a handle: inside the box, off the centre and the radius
        // handles; plus the outline point (a vertex or the box top).
        let inside = pt(centre.x + 12.0 / k, centre.y + 40.0 / k);
        for selected in [false, true] {
            for (spot, p) in [("outline", outline), ("centre", centre), ("inside", inside)] {
                // An unfilled, unselected shape is not hit inside its box.
                if !selected && spot != "outline" {
                    continue;
                }
                // The drawn centre handle of a selected primitive opens the typed
                // move instead (`edit-interaction-polish` criterion 15); covered
                // in `edit_polish_typed_skew_move.rs`.
                if selected && spot == "centre" {
                    continue;
                }
                let mut s = open_session(&d);
                if selected {
                    click(&mut s, outline, false);
                }
                let before = bytes(&s);
                let changes = change_count(&s);
                let hint = dbl(&mut s, p, p);
                let label = format!("{name} {spot} selected={selected}");
                assert!(hint, "{label}: hint chip expected");
                assert_eq!(s.tool(), Tool::Select, "{label}");
                assert!(
                    s.transform_entry().is_none(),
                    "{label}: no entry with a hint"
                );
                assert_eq!(bytes(&s), before, "{label}: nothing written");
                assert_eq!(change_count(&s), changes, "{label}");
                // Shown on every such double-click.
                assert!(dbl(&mut s, p, p), "{label}: hint again");
                assert_eq!(s.tool(), Tool::Select);
                // The selection survived: with the Select tool, Delete removes
                // the shape only if it was selected.
                {
                    let survivors = delete_selection(&mut s);
                    assert!(
                        survivors.is_empty(),
                        "{label}: the primitive stayed selected"
                    );
                }
            }
        }
    }
}

#[test]
fn ac32_the_unselected_outline_hint_is_always_shown_and_no_hint_on_empty_canvas() {
    let k = k_of(&Session::new(1));
    for (name, d, outline) in primitive_docs(k) {
        let mut s = open_session(&d);
        let before = bytes(&s);
        assert!(
            dbl(&mut s, outline, outline),
            "{name}: outline of an unselected shape"
        );
        assert_eq!(bytes(&s), before);
        assert_eq!(s.tool(), Tool::Select);
        // Empty canvas: no hint (criterion 34), nothing changes.
        let far = pt(900.0, 900.0);
        let hint = dbl(&mut s, far, far);
        assert!(!hint, "{name}: empty canvas shows no hint");
        assert_eq!(bytes(&s), before);
        assert_eq!(s.tool(), Tool::Select);
        assert!(s.transform_entry().is_none());
    }
}

#[test]
fn ac33_a_double_click_on_a_handle_opens_its_entry_without_a_tool_switch() {
    let k = k_of(&Session::new(1));
    // Rectangle, 200 x 140 px, selected: corner resize, edge resize, rotate and
    // the radius handle all open an entry.
    let (mut s, fr) = rect_scene(200.0, 140.0, 0.0);
    let before = bytes(&s);
    let handles = [
        ("corner resize", fr.corner(1.0, 1.0), "size"),
        ("edge resize", fr.mid(1.0, 0.0), "size"),
        ("corner rotate", fr.rot_corner(1.0, 1.0), "angle"),
        ("radius", fr.radius0(-1.0, -1.0), "corner-radius"),
    ];
    for (name, p, kind) in handles {
        s.set_tool(Tool::Select);
        s.pointer_hover(pt(900.0, 900.0), false, false);
        let hint = {
            s.pointer_hover(p, false, false);
            s.pointer_down(p, false);
            s.pointer_up(p, false, false);
            let h = s.double_click(p, false, false);
            s.pointer_hover(p, false, false);
            h
        };
        assert!(!hint, "{name}: no edit hint over a handle");
        assert_eq!(s.tool(), Tool::Select, "{name}: no tool switch");
        let e = s
            .transform_entry()
            .unwrap_or_else(|| panic!("{name}: entry"));
        assert_eq!(e.kind, kind, "{name}");
        s.cancel_transform_entry();
        assert_eq!(bytes(&s), before, "{name}: nothing written");
    }
    // Star inner-radius handle.
    let frame = curvyo_document_core::StarFrame {
        center: pt(100.0, 100.0),
        radius: Length::from_mm(100.0 / k),
        angle: curvyo_document_core::Angle::from_radians(-std::f64::consts::FRAC_PI_2),
    };
    let d = Document::new(1);
    let _ = d.create_star(
        frame,
        PointCount::new(8).unwrap(),
        curvyo_document_core::InnerRatio::new(0.5).unwrap(),
    );
    let mut s = open_session(&d);
    let p = outline_point(&s, 0);
    click(&mut s, p, false);
    let pts = outline_of_rotated(&prim(&s, 0).shape, prim(&s, 0).rotation);
    let inner = pts[1].point;
    s.pointer_hover(inner, false, false);
    s.pointer_down(inner, false);
    s.pointer_up(inner, false, false);
    assert!(!s.double_click(inner, false, false));
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.transform_entry().unwrap().kind, "inner-ratio");
}

#[test]
fn ac33_a_double_click_on_a_skew_handle_opens_the_skew_entry_and_never_switches_tool() {
    let k = k_of(&Session::new(1));
    let mut s = open_session(&triangle_path_doc(k));
    click(&mut s, pt(40.0 + 100.0 / k, 30.0), false);
    let before = bytes(&s);
    let skew = pt(40.0 + 100.0 / k, 30.0 - 16.0 / k);
    s.pointer_hover(skew, false, false);
    assert_eq!(
        s.handle_hint(),
        "skew",
        "precondition: this is the skew handle"
    );
    let hint = dbl(&mut s, skew, skew);
    assert!(!hint, "no hint on a skew handle");
    assert_eq!(s.tool(), Tool::Select, "no tool switch");
    assert_eq!(
        s.transform_entry().map(|entry| entry.kind),
        Some("skew"),
        "the skew handle opens the skew entry (`edit-interaction-polish` criterion 9)"
    );
    assert_eq!(bytes(&s), before);
}

#[test]
fn ac33_a_path_resize_handle_double_click_opens_an_entry_not_the_node_tool() {
    let k = k_of(&Session::new(1));
    let mut s = open_session(&triangle_path_doc(k));
    click(&mut s, pt(40.0 + 100.0 / k, 30.0), false);
    let corner = pt(40.0 + 200.0 / k, 30.0 + 140.0 / k);
    s.pointer_hover(corner, false, false);
    s.pointer_down(corner, false);
    s.pointer_up(corner, false, false);
    let hint = s.double_click(corner, false, false);
    assert!(!hint);
    assert_eq!(s.tool(), Tool::Select);
    assert!(s.transform_entry().is_some());
}

#[test]
fn ac34_double_click_on_empty_canvas_does_nothing() {
    let k = k_of(&Session::new(1));
    for d in [triangle_path_doc(k), primitive_docs(k).remove(0).1] {
        let mut s = open_session(&d);
        let before = bytes(&s);
        let far = pt(900.0, 900.0);
        assert!(!dbl(&mut s, far, far));
        assert_eq!(s.tool(), Tool::Select);
        assert_eq!(bytes(&s), before);
        assert!(s.transform_entry().is_none());
    }
}

#[test]
fn ac25_a_double_click_in_a_creation_tool_creates_nothing_and_changes_nothing() {
    let k = k_of(&Session::new(1));
    for tool in CREATION {
        let d = primitive_docs(k).remove(0).1;
        let mut s = open_session(&d);
        let o = outline_point(&s, 0);
        click(&mut s, o, false);
        s.set_tool(tool);
        let before = bytes(&s);
        for p in [o, pt(900.0, 900.0)] {
            assert!(
                !dbl(&mut s, p, p),
                "{tool:?}: no hint chip in a creation tool"
            );
            assert_eq!(s.tool(), tool);
            assert_eq!(bytes(&s), before, "{tool:?}: nothing created");
        }
    }
}

// =====================================================================
// Regression sweep: draw with each shape tool, then edit through Select
// =====================================================================

#[test]
fn sweep_draw_each_shape_then_move_resize_and_rotate_it_through_the_select_tool() {
    let k = k_of(&Session::new(1));
    for tool in CREATION {
        let mut s = Session::new(1);
        s.set_tool(tool);
        s.set_poly_star_mode(PolyStarMode::Star);
        let (a, b) = (pt(100.0, 100.0), pt(100.0 + 100.0 / k, 100.0 + 70.0 / k));
        drag(&mut s, a, b, false, false);
        assert_eq!(s.tool(), Tool::Select);
        let created = prim(&s, 0);
        let bx = box_of(&created);
        let centre = pt(f64::midpoint(bx.0, bx.2), f64::midpoint(bx.1, bx.3));
        let w = (bx.2 - bx.0) * k;
        let h = (bx.3 - bx.1) * k;
        assert!(
            w > 72.0 && h > 72.0
                || tool == Tool::PolygonStar
                || tool == Tool::Ellipse
                || tool == Tool::Rectangle
        );
        // Move: drag from just inside an edge-less spot of the box (the box
        // body between centre and edge is a move, criterion 5).
        let from = pt(centre.x + 5.0 / k, centre.y + 5.0 / k);
        let before = shape_dbg(&s, 0);
        drag(
            &mut s,
            from,
            pt(from.x + 30.0 / k, from.y + 20.0 / k),
            false,
            false,
        );
        let after = prim(&s, 0);
        let nb = box_of(&after);
        assert!(
            near(nb.0 - bx.0, 30.0 / k, 1e-6),
            "{tool:?} moved x: {before}"
        );
        assert!(near(nb.1 - bx.1, 20.0 / k, 1e-6), "{tool:?} moved y");
        assert!(
            near((nb.2 - nb.0) * k, w, 1e-6),
            "{tool:?}: size kept by a move"
        );
        assert_eq!(n_objects(&s), 1);
        // Rotate with the corner rotate handle by a quarter turn about the
        // centre.
        let c = pt(f64::midpoint(nb.0, nb.2), f64::midpoint(nb.1, nb.3));
        let hw = (nb.2 - nb.0) / 2.0 * k;
        let hh = (nb.3 - nb.1) / 2.0 * k;
        let d = 32.0 / SQRT_2;
        // The box of a polygon or star is turned by its shown angle
        // (`polygon-star-box-refit`); for the other kinds that angle is 0 here.
        let shown = curvyo_document_core::ObjectSnapshot::Primitive(prim(&s, 0))
            .orientation()
            .as_radians();
        let handle = rot(pt(c.x + (hw + d) / k, c.y + (hh + d) / k), c, shown);
        let target = rot(handle, c, std::f64::consts::FRAC_PI_2);
        let rot_before = prim(&s, 0).rotation.as_radians();
        drag(&mut s, handle, target, false, false);
        let rotated = prim(&s, 0).rotation.as_radians();
        let delta = (rotated - rot_before).rem_euclid(std::f64::consts::TAU);
        assert!(
            near(delta, std::f64::consts::FRAC_PI_2, 1e-6),
            "{tool:?}: rotated by {delta}"
        );
        assert_eq!(n_objects(&s), 1);
    }
}

#[test]
fn sweep_rectangle_radius_handle_and_pen_then_node_flows_still_work() {
    let k = k_of(&Session::new(1));
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(
        &mut s,
        pt(20.0, 20.0),
        pt(20.0 + 200.0 / k, 20.0 + 140.0 / k),
        false,
        false,
    );
    // radius handle at 0: 15 px along the diagonal from the top-left corner
    let h = pt(20.0 + 15.0 / SQRT_2 / k, 20.0 + 15.0 / SQRT_2 / k);
    let to = pt(h.x + 30.0 / k, h.y + 30.0 / k);
    drag(&mut s, h, to, false, false);
    match prim(&s, 0).shape {
        Shape::Rect { corner_radii, .. } => assert!(uniform_mm(corner_radii) > 0.0),
        other => panic!("{other:?}"),
    }
    // Pen: three clicks and a finish, no tool-switch side effects.
    s.set_tool(Tool::Pen);
    for p in [pt(300.0, 300.0), pt(400.0, 300.0), pt(400.0, 400.0)] {
        s.pointer_down(p, false);
        s.pointer_up(p, false, false);
    }
    s.finish_pen();
    assert_eq!(n_objects(&s), 2);
    assert_eq!(s.tool(), Tool::Pen, "pen stays the active tool");
    // Node tool: move one node of the path.
    s.set_tool(Tool::Node);
    let p0 = pt(300.0, 300.0);
    s.pointer_hover(p0, false, false);
    s.pointer_down(p0, false);
    s.pointer_hover(pt(310.0, 305.0), false, false);
    s.pointer_up(pt(310.0, 305.0), false, false);
    let d = doc_of(&s);
    let path = d.path(d.object_ids()[1]).unwrap();
    assert_eq!(path.anchors[0].point, pt(310.0, 305.0));
    assert_eq!(s.tool(), Tool::Node);
    // The rectangle is untouched by all of that.
    match prim(&s, 0).shape {
        Shape::Rect { bounds: r, .. } => assert!(near(r.origin.x, 20.0, 1e-9)),
        other => panic!("{other:?}"),
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
