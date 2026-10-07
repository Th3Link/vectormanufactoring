//! Session-level tests of `specs/polygon-star-box-refit/`: the box a polygon
//! or star shows, hit-tests and drags is the square turned by its shown angle.
//! They read the box from the decoration input the draw list is built from, so
//! "drawn" and "tested" are checked against the same function. Expected values
//! come from the worked examples of the specification and from the outline's
//! own geometry, never from `oriented_bounds`.

use vecmanf_document_core::{
    Angle, InnerRatio, Length, NodeId, Point, PointCount, Shape, StarFrame, outline_of_rotated,
};
use vecmanf_render_core::TransformGlyphKind;
use vecmanf_ui_core::{EditHandle, EntryOutcome, ParamHandle, ResizeDirection, SelectTool};

use super::{Session, Tool};
use crate::KeyInput;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: Point, b: Point, tol: f64) -> bool {
    (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
}

/// `a - b` wrapped into `(-pi, pi]`, in radians.
fn angle_diff(a: f64, b: f64) -> f64 {
    Angle::from_radians(a - b).normalized().as_radians()
}

fn key(session: &mut Session, k: &str) {
    session.key_down(KeyInput {
        key: k,
        ..KeyInput::default()
    });
}

/// A hexagon (or star) with centre `c`, radius `r` and frame angle `deg`
/// degrees, selected by a click on its first outer vertex.
fn selected(
    shape: impl FnOnce(StarFrame) -> Shape,
    c: Point,
    r: f64,
    deg: f64,
) -> (Session, NodeId) {
    let mut session = Session::new(1);
    let frame = StarFrame {
        center: c,
        radius: Length::from_mm(r),
        angle: Angle::from_radians(deg.to_radians()),
    };
    let id = match shape(frame) {
        Shape::Polygon { frame, point_count } => {
            session.document.create_polygon(frame, point_count)
        }
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => session
            .document
            .create_star(frame, point_count, inner_ratio),
        _ => unreachable!("polygon or star only"),
    };
    session.set_tool(Tool::Select);
    let first = pt(
        c.x + r * deg.to_radians().cos(),
        c.y + r * deg.to_radians().sin(),
    );
    session.pointer_down(first, false);
    session.pointer_up(first, false, false);
    assert_eq!(session.selection.ids(), [id], "the click selects the shape");
    (session, id)
}

fn hexagon(c: Point, r: f64, deg: f64) -> (Session, NodeId) {
    selected(
        |frame| Shape::Polygon {
            frame,
            point_count: PointCount::new(6).unwrap(),
        },
        c,
        r,
        deg,
    )
}

fn star(c: Point, r: f64, deg: f64) -> (Session, NodeId) {
    selected(
        |frame| Shape::Star {
            frame,
            point_count: PointCount::new(5).unwrap(),
            inner_ratio: InnerRatio::new(0.5).unwrap(),
        },
        c,
        r,
        deg,
    )
}

/// The drawn box of the selected shape (top-left, top-right, bottom-right,
/// bottom-left of its own frame).
fn drawn_box(session: &Session) -> [Point; 4] {
    session.select_decoration_input().selected[0].1
}

/// The direction of the drawn box's top edge, in radians.
fn box_direction(corners: &[Point; 4]) -> f64 {
    let edge = corners[0].vector_to(corners[1]);
    edge.y.atan2(edge.x)
}

fn handle_position(session: &Session, wanted: EditHandle, side_rotate: bool) -> Point {
    SelectTool::transform_handles(
        &session.objects(),
        &session.selection,
        session.transform_handle_tolerances(),
        side_rotate,
    )
    .into_iter()
    .find(|(handle, _)| *handle == wanted)
    .unwrap_or_else(|| panic!("no {wanted:?} handle"))
    .1
}

fn primitive_of(session: &Session, id: NodeId) -> vecmanf_document_core::PrimitiveSnapshot {
    session.document.primitive(id).unwrap()
}

fn frame_of(shape: &Shape) -> StarFrame {
    match shape {
        Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => *frame,
        _ => unreachable!("polygon or star only"),
    }
}

/// The number the readout shows for the first outer vertex, from geometry.
fn shown_degrees(session: &Session, id: NodeId) -> f64 {
    let p = primitive_of(session, id);
    let c = frame_of(&p.shape).center;
    let v = outline_of_rotated(&p.shape, p.rotation)[0].point;
    (v.y - c.y).atan2(v.x - c.x).to_degrees()
}

/// A press on the rotate handle of the box's NE corner, then `deg` degrees
/// clockwise about `pivot`, with a detour out of the dead zone first. Calls
/// `each` after every pointer step (live) and returns the point released at.
fn rotate_drag(
    session: &mut Session,
    deg: f64,
    pivot: Point,
    shift: bool,
    ctrl: bool,
    mut each: impl FnMut(&Session),
) -> Point {
    let handle = handle_position(session, EditHandle::Rotate(ResizeDirection::Ne), false);
    let turned = |d: f64| {
        let v = pivot.vector_to(handle);
        let end = v.y.atan2(v.x) + d.to_radians();
        let dist = v.x.hypot(v.y);
        pt(pivot.x + dist * end.cos(), pivot.y + dist * end.sin())
    };
    session.pointer_hover(handle, shift, ctrl);
    session.pointer_down(handle, shift);
    for step in [if deg >= 0.0 { 40.0 } else { -40.0 }, deg / 2.0, deg] {
        let at = turned(step);
        session.pointer_hover(at, shift, ctrl);
        each(session);
    }
    let end = turned(deg);
    session.pointer_up(end, shift, ctrl);
    end
}

fn assert_corners(got: &[Point; 4], want: [Point; 4], tol: f64) {
    for (g, w) in got.iter().zip(want) {
        assert!(near(*g, w, tol), "{got:?} vs {want:?}");
    }
}

/// Criteria 1, 2: the worked example, drawn and hovered.
#[test]
fn the_drawn_selected_and_hovered_box_is_the_turned_square() {
    let want = [
        pt(-3.66, -13.66),
        pt(13.66, -3.66),
        pt(3.66, 13.66),
        pt(-13.66, 3.66),
    ];
    let (mut session, _) = hexagon(pt(0.0, 0.0), 10.0, 30.0);
    assert_corners(&drawn_box(&session), want, 0.01);
    // The first outer vertex is on the middle of the right-hand side.
    let first = pt(
        10.0 * 30.0_f64.to_radians().cos(),
        10.0 * 30.0_f64.to_radians().sin(),
    );
    let mid = pt(
        f64::midpoint(want[1].x, want[2].x),
        f64::midpoint(want[1].y, want[2].y),
    );
    assert!(near(first, mid, 0.01));

    // Hovered: deselect, then hover the outline.
    session.selection.clear();
    session.pointer_hover(first, false, false);
    let hovered = session.select_decoration_input().hovered.expect("hovered");
    assert_corners(&hovered.1, want, 0.01);
}

/// Criterion 1 at 0 degrees, and on a star.
#[test]
fn an_upright_shape_has_an_axis_aligned_box() {
    let (session, _) = star(pt(5.0, 5.0), 10.0, 0.0);
    assert_corners(
        &drawn_box(&session),
        [
            pt(-5.0, -5.0),
            pt(15.0, -5.0),
            pt(15.0, 15.0),
            pt(-5.0, 15.0),
        ],
        1e-9,
    );
}

/// Criterion 11: the cursor arrows follow the turned box.
#[test]
fn resize_cursors_follow_the_turned_box() {
    let (mut session, _) = hexagon(pt(0.0, 0.0), 10.0, 30.0);
    let ne = handle_position(&session, EditHandle::Resize(ResizeDirection::Ne), false);
    let se = handle_position(&session, EditHandle::Resize(ResizeDirection::Se), false);
    session.pointer_hover(ne, false, false);
    assert_eq!(session.cursor_hint(), "resize:165.0");
    session.pointer_hover(se, false, false);
    assert_eq!(session.cursor_hint(), "resize:75.0");
}

/// Criterion 15: a rectangle's cursor is as before (rotation register).
#[test]
fn a_rotated_rectangles_cursor_is_unchanged() {
    let mut session = Session::new(1);
    let id = session
        .document
        .create_rect(vecmanf_document_core::RectBounds {
            origin: pt(0.0, 0.0),
            width: Length::from_mm(20.0),
            height: Length::from_mm(20.0),
        });
    let rotated = session
        .document
        .object(id)
        .unwrap()
        .rotated(pt(10.0, 10.0), Angle::from_radians(30.0_f64.to_radians()));
    session.document.rotate_object(&rotated).unwrap();
    session.set_tool(Tool::Select);
    let corner = rotated_corner(pt(10.0, 10.0), pt(0.0, 0.0), 30.0);
    session.pointer_down(corner, false);
    session.pointer_up(corner, false, false);
    let ne = handle_position(&session, EditHandle::Resize(ResizeDirection::Ne), false);
    session.pointer_hover(ne, false, false);
    assert_eq!(session.cursor_hint(), "resize:165.0");
}

fn rotated_corner(pivot: Point, p: Point, deg: f64) -> Point {
    p.rotated_around(pivot, Angle::from_radians(deg.to_radians()))
}

/// Criterion 12: the move region is the turned box. The probe points are far
/// from the outline and from every handle, so only the box decides.
#[test]
fn a_press_inside_the_turned_box_moves_the_shape_and_one_outside_does_not() {
    // R = 30 at 30 degrees: (31.7, 1.0) is the box-local point (28, -15), inside
    // the turned box and right of the axis-aligned one, 5.7 mm from the outline.
    let (mut session, id) = hexagon(pt(0.0, 0.0), 30.0, 30.0);
    let inside = pt(31.7, 1.0);
    session.pointer_hover(inside, false, false);
    session.pointer_down(inside, false);
    session.pointer_hover(pt(36.7, 1.0), false, false);
    session.pointer_up(pt(36.7, 1.0), false, false);
    let moved = frame_of(&primitive_of(&session, id).shape).center;
    assert!(near(moved, pt(5.0, 0.0), 1e-9), "{moved:?}");

    // (-29, -29) is inside the axis-aligned box and outside the turned one.
    let (mut session, id) = hexagon(pt(0.0, 0.0), 30.0, 30.0);
    session.pointer_hover(pt(-29.0, -29.0), false, false);
    session.pointer_down(pt(-29.0, -29.0), false);
    session.pointer_hover(pt(-24.0, -29.0), false, false);
    session.pointer_up(pt(-24.0, -29.0), false, false);
    let still = frame_of(&primitive_of(&session, id).shape).center;
    assert!(near(still, pt(0.0, 0.0), 1e-9), "{still:?}");
}

/// Criterion 10: no skew handle at any angle, drawn or hit-testable.
#[test]
fn no_skew_handle_exists_at_any_angle() {
    for deg in [0.0, 30.0, 78.7, -90.0, 180.0] {
        for (mut session, _) in [
            hexagon(pt(0.0, 0.0), 10.0, deg),
            star(pt(0.0, 0.0), 10.0, deg),
        ] {
            for side_rotate in [false, true] {
                let handles = SelectTool::transform_handles(
                    &session.objects(),
                    &session.selection,
                    session.transform_handle_tolerances(),
                    side_rotate,
                );
                assert!(
                    handles
                        .iter()
                        .all(|(h, _)| !matches!(h, EditHandle::Skew(_))),
                    "{deg}: {handles:?}"
                );
            }
            let drawn = session.select_transform_decoration_input().handles;
            assert!(
                drawn
                    .iter()
                    .all(|g| !matches!(g.kind, TransformGlyphKind::Skew { .. })),
                "{deg}"
            );
            key(&mut session, "k");
            assert!(
                session.transform_entry().is_none(),
                "no skew entry at {deg}"
            );
        }
    }
}

/// Criteria 5, 9: at every step of a corner-rotate drag the live box has the
/// direction of the live readout; the committed box is the last live box.
#[test]
fn a_rotate_drag_keeps_box_and_readout_the_same_angle_and_commits_the_last_live_box() {
    for shift in [false, true] {
        let c = pt(0.0, 0.0);
        let (mut session, id) = hexagon(c, 10.0, 78.7);
        let start_box = drawn_box(&session);
        // Shift: the corner opposite the grabbed NE rotate handle (SW).
        let pivot = if shift { start_box[3] } else { c };
        let mut last = start_box;
        let mut seen = 0;
        rotate_drag(&mut session, 20.0, pivot, shift, false, |s| {
            let readout = s.live_readout().map(|r| r.text);
            let Some(text) = readout else { return };
            let shown: f64 = text
                .trim_end_matches('°')
                .replace('\u{2212}', "-")
                .parse()
                .unwrap();
            let live = drawn_box(s);
            assert!(
                angle_diff(box_direction(&live), shown.to_radians()).abs() < 1e-3,
                "box {} vs readout {shown}",
                box_direction(&live).to_degrees()
            );
            if shift {
                let marker = s
                    .select_transform_decoration_input()
                    .pivot_marker
                    .expect("marker");
                assert!(
                    near(marker, pivot, 1e-9),
                    "pivot marker {marker:?} vs {pivot:?}"
                );
            }
            last = live;
            seen += 1;
        });
        assert!(seen >= 2, "live frames seen: {seen}");
        let committed = drawn_box(&session);
        for (c0, l0) in committed.iter().zip(last) {
            assert!(near(*c0, l0, 1e-9), "commit {committed:?} vs live {last:?}");
        }
        // The shown angle moved by the swept angle; the box turned with it.
        let shown = shown_degrees(&session, id);
        assert!(
            angle_diff(box_direction(&committed), shown.to_radians()).abs() < 1e-9,
            "box {} vs shape {shown}",
            box_direction(&committed).to_degrees()
        );
        assert!((angle_diff(shown.to_radians(), (78.7_f64 + 20.0).to_radians())).abs() < 1e-3);
    }
}

/// Criterion 6: with Ctrl the shown angle snaps and the box has exactly that
/// direction.
#[test]
fn a_ctrl_rotate_lands_the_box_on_the_snapped_angle() {
    let (mut session, id) = hexagon(pt(0.0, 0.0), 10.0, 78.7);
    rotate_drag(&mut session, 1.0, pt(0.0, 0.0), false, true, |_| {});
    let shown = shown_degrees(&session, id);
    assert!((shown - 75.0).abs() < 1e-9, "{shown}");
    let direction = box_direction(&drawn_box(&session));
    assert!(
        angle_diff(direction, 75.0_f64.to_radians()).abs() < 1e-12,
        "{direction}"
    );
}

/// Criterion 7: the corner resize example.
#[test]
fn a_corner_resize_follows_the_pointer_and_keeps_the_angle() {
    let (mut session, id) = hexagon(pt(0.0, 0.0), 10.0, 30.0);
    let ne = handle_position(&session, EditHandle::Resize(ResizeDirection::Ne), false);
    assert!(near(ne, pt(13.66, -3.66), 0.01), "{ne:?}");
    let to = pt(ne.x + 4.83, ne.y - 1.29);
    session.pointer_hover(ne, false, false);
    session.pointer_down(ne, false);
    session.pointer_hover(pt(ne.x + 1.0, ne.y - 0.3), false, false);
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
    let p = primitive_of(&session, id);
    let frame = frame_of(&p.shape);
    assert!(
        (frame.radius.as_mm() - 13.54).abs() < 0.01,
        "{}",
        frame.radius.as_mm()
    );
    assert!(near(frame.center, pt(0.0, 0.0), 1e-9));
    assert!((shown_degrees(&session, id) - 30.0).abs() < 1e-6);
    // The box keeps the direction and the dragged handle sits under the pointer.
    assert!(angle_diff(box_direction(&drawn_box(&session)), 30.0_f64.to_radians()).abs() < 1e-9);
    let ne_after = handle_position(&session, EditHandle::Resize(ResizeDirection::Ne), false);
    assert!(near(ne_after, to, 0.01), "{ne_after:?} vs {to:?}");
}

/// Criterion 4: R, 0, Enter turns the box upright; a table of typed angles.
#[test]
fn a_typed_angle_gives_the_box_that_direction() {
    for typed in [-135.0, -90.0, 0.0, 15.0, 45.0, 90.0, 180.0] {
        let (mut session, id) = hexagon(pt(3.0, 4.0), 10.0, 78.7);
        key(&mut session, "r");
        let outcome = session.commit_transform_entry(&format!("{typed}"), "", 0);
        assert_eq!(outcome, EntryOutcome::Committed, "{typed}");
        let direction = box_direction(&drawn_box(&session));
        assert!(
            angle_diff(direction, f64::to_radians(typed)).abs() < 1e-9,
            "typed {typed}: box {}",
            direction.to_degrees()
        );
        assert!(
            angle_diff(
                shown_degrees(&session, id).to_radians(),
                f64::to_radians(typed)
            )
            .abs()
                < 1e-9
        );
    }
    let (mut session, _) = hexagon(pt(0.0, 0.0), 10.0, 78.7);
    key(&mut session, "r");
    session.commit_transform_entry("0", "", 0);
    assert_corners(
        &drawn_box(&session),
        [
            pt(-10.0, -10.0),
            pt(10.0, -10.0),
            pt(10.0, 10.0),
            pt(-10.0, 10.0),
        ],
        1e-9,
    );
    key(&mut session, "r");
    assert_eq!(session.transform_entry().unwrap().fields[0].prefill, "0");
}

/// Criterion 8: the star's handle is on the real first inner vertex at several
/// angles, and a point-count change keeps the box.
#[test]
fn the_star_inner_handle_sits_on_the_inner_vertex_at_every_angle() {
    for deg in [0.0, 30.0, 78.7, -90.0, 180.0, -170.0] {
        let (mut session, id) = star(pt(10.0, -5.0), 30.0, deg);
        let handle = handle_position(&session, EditHandle::Param(ParamHandle::InnerRadius), false);
        let p = primitive_of(&session, id);
        let inner = outline_of_rotated(&p.shape, p.rotation)[1].point;
        assert!(near(handle, inner, 1e-9), "{deg}: {handle:?} vs {inner:?}");
        let before = drawn_box(&session);
        session.set_selected_point_count(PointCount::new(9).unwrap());
        assert_corners(&drawn_box(&session), before, 1e-9);
    }
}

/// Criterion 3: a shape just made by the create-drag has the box of its angle.
#[test]
fn a_create_drag_selects_a_shape_whose_box_has_the_drag_direction() {
    let mut session = Session::new(1);
    session.set_tool(Tool::PolygonStar);
    session.set_poly_star_point_count(PointCount::new(6).unwrap());
    let (a, b) = (pt(0.0, 0.0), pt(8.66, 5.0));
    session.pointer_hover(a, false, false);
    session.pointer_down(a, false);
    session.pointer_hover(b, false, false);
    session.pointer_up(b, false, false);
    assert_eq!(session.tool, Tool::Select);
    assert_corners(
        &drawn_box(&session),
        [
            pt(-3.66, -13.66),
            pt(13.66, -3.66),
            pt(3.66, 13.66),
            pt(-13.66, 3.66),
        ],
        0.01,
    );
}
