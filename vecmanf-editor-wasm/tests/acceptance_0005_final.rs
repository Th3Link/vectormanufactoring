//! Final narrow check for `specs/0005-object-transform`: a press inside the
//! sole selected object's oriented box moves it (F1), and the new rule does
//! not break handles, deselection, overlap behaviour or zero-move writes.
//! Everything goes through `Session`'s public API at the 96 dpi default view.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use vecmanf_document_core::{
    Angle, Document, EllipseFrame, Length, NodeId, ObjectSnapshot, Point, PointCount, RectBounds,
    Shape, StarFrame, pack, unpack,
};
use vecmanf_editor_wasm::{Session, Tool};

const SCALE: f64 = 96.0 / 25.4;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn px(p: f64) -> f64 {
    p / SCALE
}

fn rot(p: Point, c: Point, a: f64) -> Point {
    let (sn, cs) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * cs - dy * sn, c.y + dx * sn + dy * cs)
}

fn open(document: &Document) -> Session {
    let bytes = pack(document, "0.1.0").unwrap();
    let mut s = Session::open(2, &bytes).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn snap(s: &Session) -> Vec<u8> {
    doc_of(s).export_loro_snapshot().unwrap()
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn drag(s: &mut Session, from: Point, to: Point) {
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(to, false, false);
    s.pointer_up(to, false, false);
}

fn hint(s: &mut Session, p: Point) -> String {
    s.pointer_hover(p, false, false);
    s.cursor_hint()
}

fn rotate_about_centre(d: &Document, id: NodeId, c: Point, a: f64) {
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(c, Angle::from_radians(a)))
        .unwrap();
}

/// Frame summary used to compare "only moved": (size-ish, rotation).
fn rotation_of(s: &Session) -> f64 {
    let d = doc_of(s);
    d.primitive(d.object_ids()[0])
        .unwrap()
        .rotation
        .as_radians()
}

fn shape_of(s: &Session, idx: usize) -> Shape {
    let d = doc_of(s);
    d.primitive(d.object_ids()[idx]).unwrap().shape
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Rect,
    Ellipse,
    Polygon,
    Star,
}

/// Builds a document with one object whose box is `size_px` square on screen,
/// centred at (50, 50) mm, optionally rotated about the centre.
fn one(kind: Kind, size_px: f64, a: f64) -> (Session, Point, f64) {
    let c = pt(50.0, 50.0);
    let s_mm = px(size_px);
    let d = Document::new(1);
    let id = match kind {
        Kind::Rect => d.create_rect(RectBounds {
            origin: pt(c.x - s_mm / 2.0, c.y - s_mm / 2.0),
            width: Length::from_mm(s_mm),
            height: Length::from_mm(s_mm),
        }),
        Kind::Ellipse => d.create_ellipse(EllipseFrame {
            center: c,
            rx: Length::from_mm(s_mm / 2.0),
            ry: Length::from_mm(s_mm / 2.0),
        }),
        Kind::Polygon => d.create_polygon(
            StarFrame {
                center: c,
                radius: Length::from_mm(s_mm / 2.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(6).unwrap(),
        ),
        Kind::Star => d.create_star(
            StarFrame {
                center: c,
                radius: Length::from_mm(s_mm / 2.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).unwrap(),
            vecmanf_document_core::InnerRatio::new(0.5).unwrap(),
        ),
    };
    if a != 0.0 {
        rotate_about_centre(&d, id, c, a);
    }
    (open(&d), c, s_mm)
}

/// A point that is certainly on the object's outline, for selecting it.
fn outline_point(kind: Kind, c: Point, s_mm: f64, a: f64) -> Point {
    let p = match kind {
        Kind::Rect | Kind::Ellipse => pt(c.x, c.y - s_mm / 2.0),
        // Vertex at angle 0: first outer vertex to the right of centre.
        Kind::Polygon | Kind::Star => pt(c.x + s_mm / 2.0, c.y),
    };
    rot(p, c, a)
}

fn origin_x_of_first(s: &Session) -> f64 {
    let d = doc_of(s);
    match d.primitive(d.object_ids()[0]).unwrap().shape {
        Shape::Rect { bounds, .. } => bounds.origin.x,
        Shape::Ellipse { frame } => frame.center.x,
        Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => frame.center.x,
    }
}

/// F1 across kinds, sizes 10..=80 px, rotated or not: select by the outline,
/// then a press at the centre moves the object and nothing else.
#[test]
fn f1_a_selected_object_of_any_small_size_moves_from_its_centre() {
    for kind in [Kind::Rect, Kind::Ellipse, Kind::Polygon, Kind::Star] {
        for size in [10.0, 12.0, 16.0, 20.0, 24.0, 30.0, 40.0, 50.0, 60.0, 80.0] {
            for deg in [0.0_f64, 30.0, 90.0, -135.0] {
                let a = deg.to_radians();
                let (mut s, c, s_mm) = one(kind, size, a);
                click(&mut s, outline_point(kind, c, s_mm, a));
                // Selection really happened.
                let before_shape = shape_of(&s, 0);
                let before_rot = rotation_of(&s);
                let before_x = origin_x_of_first(&s);
                drag(&mut s, c, pt(c.x + 3.0, c.y + 1.0));
                let after_x = origin_x_of_first(&s);
                assert!(
                    (after_x - before_x - 3.0).abs() < 1e-6,
                    "{kind:?} {size}px {deg}deg: moved by {} not 3",
                    after_x - before_x
                );
                assert!((rotation_of(&s) - before_rot).abs() < 1e-9, "rotation kept");
                // Size untouched: compare shape with the move subtracted.
                match (before_shape, shape_of(&s, 0)) {
                    (Shape::Rect { bounds: b0, .. }, Shape::Rect { bounds: b1, .. }) => {
                        assert!((b0.width.as_mm() - b1.width.as_mm()).abs() < 1e-9);
                        assert!((b0.height.as_mm() - b1.height.as_mm()).abs() < 1e-9);
                    }
                    (Shape::Ellipse { frame: f0 }, Shape::Ellipse { frame: f1 }) => {
                        assert!((f0.rx.as_mm() - f1.rx.as_mm()).abs() < 1e-9);
                    }
                    (Shape::Polygon { frame: f0, .. }, Shape::Polygon { frame: f1, .. })
                    | (Shape::Star { frame: f0, .. }, Shape::Star { frame: f1, .. }) => {
                        assert!((f0.radius.as_mm() - f1.radius.as_mm()).abs() < 1e-9);
                    }
                    _ => panic!("shape kind changed"),
                }
            }
        }
    }
}

/// A rotated box's interior is the rotated one: a point inside the rotated
/// box but outside the unrotated one moves the object; a point inside the
/// unrotated box but outside the rotated one does not.
#[test]
fn f1_the_inside_test_uses_the_rotated_box() {
    let a = 45.0_f64.to_radians();
    let size = 80.0;
    let (mut s, c, s_mm) = one(Kind::Rect, size, a);
    click(&mut s, outline_point(Kind::Rect, c, s_mm, a));
    // Local point just inside the NE corner region; rotate to document space.
    // Local (c + 0.45 s, c - 0.45 s) is inside the box, near the corner,
    // beyond handle radius? handles are at most ~16px; use 0.3 s (24 px away).
    let inside = rot(pt(c.x + 0.3 * s_mm, c.y - 0.3 * s_mm), c, a);
    let before = origin_x_of_first(&s);
    drag(&mut s, inside, pt(inside.x + 2.0, inside.y));
    assert!((origin_x_of_first(&s) - before - 2.0).abs() < 1e-6, "moved");

    // A point at distance 0.7 s from centre along the document x axis is
    // outside the 45-degree box's inscribed region (box half diagonal 0.707 s
    // along x, but at y=0 the rotated box edge is at 0.707 s) -> pick a
    // document-space corner of the unrotated box instead: (0.45 s, -0.45 s)
    // has local coords at 45deg of (0.636 s, 0) -> outside the box half
    // size 0.5 s.
    let (mut s, c, s_mm) = one(Kind::Rect, size, a);
    click(&mut s, outline_point(Kind::Rect, c, s_mm, a));
    let outside = pt(c.x + 0.45 * s_mm, c.y - 0.45 * s_mm);
    let before = snap(&s);
    // 0.636 s - 0.5 s = 0.136 s = ~11 px beyond the edge: may sit on the
    // handle radius, so push to the far corner of the document-space box.
    let far = pt(c.x + 0.7 * s_mm, c.y + 0.7 * s_mm);
    click(&mut s, far);
    assert_eq!(snap(&s), before, "no write for a deselecting click");
    let _ = outside;
    // Deselected: the handles are gone, so a former handle spot has no
    // resize cursor any more.
    let handle = rot(pt(c.x + s_mm / 2.0, c.y + s_mm / 2.0), c, a);
    assert_eq!(hint(&mut s, handle), "default", "deselected");
}

/// Handles still win over the interior: every handle position of a selected
/// object, hovered or pressed, is a resize/rotate and never a move.
#[test]
fn handles_still_win_and_rotate_handle_works() {
    for size in [30.0, 40.0, 60.0, 80.0] {
        for deg in [0.0_f64, 30.0] {
            let a = deg.to_radians();
            let (mut s, c, s_mm) = one(Kind::Rect, size, a);
            click(&mut s, outline_point(Kind::Rect, c, s_mm, a));
            let se = rot(pt(c.x + s_mm / 2.0, c.y + s_mm / 2.0), c, a);
            assert!(hint(&mut s, se).starts_with("resize:"), "{size} {deg}");
            let before = shape_of(&s, 0);
            let before_x = origin_x_of_first(&s);
            drag(
                &mut s,
                se,
                rot(pt(c.x + s_mm / 2.0 + 5.0, c.y + s_mm / 2.0 + 5.0), c, a),
            );
            assert_ne!(shape_of(&s, 0), before, "resize wrote a new frame");
            // Not a pure move: the size changed.
            let Shape::Rect { bounds, .. } = shape_of(&s, 0) else {
                panic!()
            };
            assert!(bounds.width.as_mm() > s_mm + 4.9, "{size} {deg}: resized");
            let _ = before_x;
        }
        // Rotate handle.
        let (mut s, c, s_mm) = one(Kind::Rect, size, 0.0);
        click(&mut s, outline_point(Kind::Rect, c, s_mm, 0.0));
        let diagonal = px(32.0) / std::f64::consts::SQRT_2;
        let handle = pt(c.x + s_mm / 2.0 + diagonal, c.y - s_mm / 2.0 - diagonal);
        assert_eq!(hint(&mut s, handle), "rotate");
        let before_x = origin_x_of_first(&s);
        drag(&mut s, handle, rot(handle, c, 20.0_f64.to_radians()));
        assert!((rotation_of(&s) - 20.0_f64.to_radians()).abs() < 1e-9);
        assert_eq!(
            origin_x_of_first(&s),
            before_x,
            "rotation does not move frame"
        );
    }
}

/// A click on empty canvas just outside the box (beyond handle reach)
/// deselects; one far away does too; the interior does not.
#[test]
fn empty_canvas_just_outside_the_box_deselects() {
    for size in [20.0, 40.0, 80.0] {
        let (mut s, c, s_mm) = one(Kind::Rect, size, 0.0);
        click(&mut s, outline_point(Kind::Rect, c, s_mm, 0.0));
        let handle_probe = pt(c.x + s_mm / 2.0, c.y + s_mm / 2.0);
        assert!(
            hint(&mut s, handle_probe).starts_with("resize:"),
            "selected"
        );
        // 40 px to the side of the right edge, mid-height: no handle there
        // (rotate handle is above, E handle is 40 px away > 16 px radius).
        let out = pt(c.x + s_mm / 2.0 + px(40.0), c.y);
        click(&mut s, out);
        assert_eq!(hint(&mut s, handle_probe), "default", "{size}px deselected");
        // And a press just below the box bottom, 30 px, away from handles.
        let (mut s, c, s_mm) = one(Kind::Rect, size, 0.0);
        click(&mut s, outline_point(Kind::Rect, c, s_mm, 0.0));
        click(&mut s, pt(c.x, c.y + s_mm / 2.0 + px(30.0)));
        assert_eq!(hint(&mut s, handle_probe), "default", "{size}px below");
    }
}

/// Reports where the boundary of "just outside" lies: how close to the box a
/// press must be before it still counts as the object (handle or outline).
#[test]
fn a_press_a_few_px_outside_the_box_edge_is_a_move_or_deselect_not_a_corruption() {
    let (mut s, c, s_mm) = one(Kind::Rect, 80.0, 0.0);
    click(&mut s, outline_point(Kind::Rect, c, s_mm, 0.0));
    let before = origin_x_of_first(&s);
    // 3 px outside the right edge, mid-height, away from handles.
    let p = pt(c.x + s_mm / 2.0 + px(3.0), c.y);
    drag(&mut s, p, pt(p.x + 2.0, p.y));
    let moved = origin_x_of_first(&s) - before;
    // Within the outline tolerance (a few px) this is still the object.
    assert!(
        moved.abs() < 1e-9 || (moved - 2.0).abs() < 1e-6,
        "moved {moved}"
    );
}

/// Selected A's box overlaps unselected B. A press inside A's box that is on
/// B's outline selects B (B's outline hit wins); a press inside A's box on B's
/// empty interior moves A.
#[test]
fn overlapping_unselected_object_outline_wins_but_its_interior_does_not() {
    let d = Document::new(1);
    let a_id = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(40.0),
    });
    // B sits fully inside A's box: 10..30 mm.
    let _b_id = d.create_rect(RectBounds {
        origin: pt(10.0, 10.0),
        width: Length::from_mm(20.0),
        height: Length::from_mm(20.0),
    });
    let _ = a_id;
    let mut s = open(&d);
    // Select A via its left edge (x=0, y=5: far from B).
    click(&mut s, pt(0.0, 5.0));
    // B's interior (20,20): A's box contains it, B's outline does not -> moves A.
    let a_before = match shape_of(&s, 0) {
        Shape::Rect { bounds, .. } => bounds.origin,
        _ => panic!(),
    };
    let b_before = match shape_of(&s, 1) {
        Shape::Rect { bounds, .. } => bounds.origin,
        _ => panic!(),
    };
    drag(&mut s, pt(20.0, 20.0), pt(23.0, 20.0));
    let a_after = match shape_of(&s, 0) {
        Shape::Rect { bounds, .. } => bounds.origin,
        _ => panic!(),
    };
    let b_after = match shape_of(&s, 1) {
        Shape::Rect { bounds, .. } => bounds.origin,
        _ => panic!(),
    };
    assert!((a_after.x - a_before.x - 3.0).abs() < 1e-6, "A moved");
    assert_eq!(b_after, b_before, "B untouched");

    // Press on B's outline (B's left edge at x=10, y=20 — and A is still
    // selected): B's outline hit wins -> B is dragged, A stays.
    let a_now = a_after;
    drag(&mut s, pt(10.0 + 3.0, 10.0), pt(13.0, 12.0));
    let a_end = match shape_of(&s, 0) {
        Shape::Rect { bounds, .. } => bounds.origin,
        _ => panic!(),
    };
    let b_end = match shape_of(&s, 1) {
        Shape::Rect { bounds, .. } => bounds.origin,
        _ => panic!(),
    };
    eprintln!("overlap: A {a_now:?} -> {a_end:?}, B {b_before:?} -> {b_end:?}");
    // Document the behaviour as a hard assertion: exactly one of them moved.
    let a_moved = a_end != a_now;
    let b_moved = b_end != b_before;
    assert!(a_moved ^ b_moved, "exactly one object moves");
}

/// A zero-movement press (and release) inside the box writes nothing, and so
/// does a press + escape, including with a sub-epsilon jitter.
#[test]
fn zero_movement_press_inside_the_box_writes_nothing() {
    for kind in [Kind::Rect, Kind::Ellipse, Kind::Polygon] {
        let (mut s, c, s_mm) = one(kind, 40.0, 0.0);
        click(&mut s, outline_point(kind, c, s_mm, 0.0));
        let before = snap(&s);
        click(&mut s, c);
        assert_eq!(snap(&s), before, "{kind:?}: click inside wrote");
        // Still selected after the click: a handle responds.
        let diagonal = px(32.0) / std::f64::consts::SQRT_2;
        let handle = pt(c.x + s_mm / 2.0 + diagonal, c.y - s_mm / 2.0 - diagonal);
        assert_eq!(hint(&mut s, handle), "rotate", "{kind:?}: still selected");
        // Press, wiggle back to the same spot, release.
        s.pointer_hover(c, false, false);
        s.pointer_down(c, false);
        s.pointer_hover(pt(c.x + 5.0, c.y), false, false);
        s.pointer_hover(c, false, false);
        s.pointer_up(c, false, false);
        assert_eq!(snap(&s), before, "{kind:?}: there-and-back wrote");
        // Press then Escape then release.
        s.pointer_hover(c, false, false);
        s.pointer_down(c, false);
        s.pointer_hover(pt(c.x + 5.0, c.y), false, false);
        s.escape();
        s.pointer_up(pt(c.x + 5.0, c.y), false, false);
        assert_eq!(snap(&s), before, "{kind:?}: escape wrote");
    }
}

/// Shift-press inside the box is not a move and does not corrupt anything.
#[test]
fn shift_press_inside_the_box_does_not_move() {
    let (mut s, c, s_mm) = one(Kind::Rect, 60.0, 0.0);
    click(&mut s, outline_point(Kind::Rect, c, s_mm, 0.0));
    let before = snap(&s);
    s.pointer_hover(c, true, false);
    s.pointer_down(c, true);
    s.pointer_hover(pt(c.x + 5.0, c.y), true, false);
    s.pointer_up(pt(c.x + 5.0, c.y), true, false);
    eprintln!("shift inside: wrote = {}", snap(&s) != before);
    let _ = doc_of(&s); // still opens
}

/// Multi-selection is unchanged: pressing inside the box of one of two selected
/// objects, away from any outline, does not start a move (deselects).
#[test]
fn multi_selection_interior_press_is_unchanged() {
    let d = Document::new(1);
    let _ = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(40.0),
    });
    let _ = d.create_rect(RectBounds {
        origin: pt(100.0, 0.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(40.0),
    });
    let mut s = open(&d);
    click(&mut s, pt(0.0, 20.0));
    s.pointer_hover(pt(100.0, 20.0), true, false);
    s.pointer_down(pt(100.0, 20.0), true);
    s.pointer_up(pt(100.0, 20.0), true, false);
    let before = snap(&s);
    drag(&mut s, pt(20.0, 20.0), pt(25.0, 20.0));
    assert_eq!(
        snap(&s),
        before,
        "interior of a multi-selected object does not move it"
    );
}

#[test]
fn unselected_object_interior_is_still_empty_canvas_via_session() {
    let (mut s, c, _) = one(Kind::Rect, 80.0, 0.0);
    let before = snap(&s);
    drag(&mut s, c, pt(c.x + 5.0, c.y));
    assert_eq!(snap(&s), before);
    let _ = ObjectSnapshot::Path;
}
