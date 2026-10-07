//! `Session`-level tests of Part A of `specs/edit-interaction-polish/
//! specification.md` (criteria 1 to 8): the angle of a created polygon or
//! star is its real orientation (`StarFrame.angle + rotation`), the create
//! readout shows it, Ctrl snaps it, a typed angle and a Ctrl rotate land on
//! the shown angle, and old files open unchanged. Driven through `Session`'s
//! public API only; the arithmetic itself is tested in `vecmanf-ui-core` and
//! `vecmanf-document-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use vecmanf_document_core::{
    Angle, CURRENT_FORMAT_VERSION, Document, InnerRatio, Length, ObjectSnapshot, Point, PointCount,
    PrimitiveSnapshot, Shape, StarFrame, outline_of_rotated, pack, unpack,
};
use vecmanf_editor_wasm::{Session, Tool};
use vecmanf_ui_core::EntryOutcome;

/// Screen pixels per millimetre of a fresh session (96 dpi at 100 %).
const SCALE: f64 = 96.0 / 25.4;

/// The corner rotate handle sits 32 px out along the box diagonal.
const CORNER_OFFSET_PX: f64 = 32.0 / std::f64::consts::SQRT_2;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn polar(center: Point, radius: f64, degrees: f64) -> Point {
    let radians = degrees.to_radians();
    pt(
        center.x + radius * radians.cos(),
        center.y + radius * radians.sin(),
    )
}

fn object(session: &Session) -> ObjectSnapshot {
    let document = unpack(99, &session.pack("0.1.0").expect("pack")).expect("unpack");
    document.object(document.object_ids()[0]).expect("object")
}

fn primitive(session: &Session) -> PrimitiveSnapshot {
    let ObjectSnapshot::Primitive(primitive) = object(session) else {
        panic!("a primitive");
    };
    primitive
}

fn frame_of(shape: Shape) -> StarFrame {
    let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = shape else {
        panic!("a polygon or star");
    };
    frame
}

fn orientation_deg(session: &Session) -> f64 {
    object(session).orientation().as_radians().to_degrees()
}

/// A create-drag from `from` to `to` with the given Ctrl state held from the
/// move on; the readout seen just before the release is returned.
fn create_drag(session: &mut Session, from: Point, to: Point, ctrl: bool) -> Option<String> {
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, ctrl);
    let readout = session.live_readout().map(|r| r.text);
    session.pointer_up(to, false, ctrl);
    readout
}

fn polygon_session(count: u32) -> Session {
    let mut session = Session::new(1);
    session.set_tool(Tool::PolygonStar);
    session.set_poly_star_point_count(PointCount::new(count).unwrap());
    session
}

/// The NE rotate handle of a shape of radius `r` at `c` whose shown angle is
/// `shown_degrees`: its box is turned by that angle
/// (`specs/polygon-star-box-refit/`).
fn ne_rotate(c: Point, r: f64, shown_degrees: f64) -> Point {
    let out = r + CORNER_OFFSET_PX / SCALE;
    let (sin, cos) = shown_degrees.to_radians().sin_cos();
    pt(c.x + out * cos + out * sin, c.y + out * sin - out * cos)
}

fn double_click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
    session.double_click(at, false, false);
}

fn type_angle(session: &mut Session, c: Point, r: f64, text: &str) -> EntryOutcome {
    let shown = orientation_deg(session);
    double_click(session, ne_rotate(c, r, shown));
    session.commit_transform_entry(text, "", 0)
}

fn drag(session: &mut Session, from: Point, to: Point, ctrl: bool) {
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, ctrl);
    session.pointer_up(to, false, ctrl);
}

/// Criterion 3: dragging straight right gives 0, down 90, up -90, and the
/// Select tool's readouts agree after the release (they showed 0 before).
#[test]
fn a_created_shape_shows_the_direction_it_was_dragged() {
    for (to, degrees) in [
        (pt(110.0, 50.0), 0.0),
        (pt(100.0, 60.0), 90.0),
        (pt(100.0, 40.0), -90.0),
        (pt(90.0, 50.0), 180.0),
    ] {
        let mut session = polygon_session(5);
        let readout = create_drag(&mut session, pt(100.0, 50.0), to, false).expect("a readout");
        assert!(
            readout.starts_with("r 10.0 mm, "),
            "readout {readout:?} after the radius"
        );
        assert_eq!(session.tool(), Tool::Select);
        assert!(
            (orientation_deg(&session) - degrees).abs() < 1e-9,
            "{to:?}: {}",
            orientation_deg(&session)
        );
        // The angle entry opens on it.
        double_click(&mut session, ne_rotate(pt(100.0, 50.0), 10.0, degrees));
        let entry = session.transform_entry().expect("an angle entry");
        assert_eq!(entry.kind, "angle");
        let shown: f64 = entry.fields[0].prefill.parse().unwrap();
        assert!(
            (shown - degrees).abs() < 1e-9,
            "prefill {}",
            entry.fields[0].prefill
        );
    }
}

/// Criteria 3 and 6: the create readout carries the angle after the radius,
/// "r 12.0 mm, -15°" for a polygon and "r 12.0 mm, ratio 0.50, -15°" for a
/// star, one decimal at most.
#[test]
fn the_create_readout_shows_the_angle() {
    let mut polygon = polygon_session(5);
    let text = create_drag(&mut polygon, pt(100.0, 50.0), pt(110.0, 48.0), true).unwrap();
    assert_eq!(text, "r 10.2 mm, -15°");
    let mut free = polygon_session(5);
    let text = create_drag(&mut free, pt(100.0, 50.0), pt(110.0, 48.0), false).unwrap();
    assert_eq!(text, "r 10.2 mm, -11.3°");
    let mut star = polygon_session(5);
    star.set_poly_star_mode(vecmanf_ui_core::PolyStarMode::Star);
    star.set_poly_star_ratio(InnerRatio::new(0.5).unwrap());
    let text = create_drag(&mut star, pt(100.0, 50.0), pt(110.0, 48.0), true).unwrap();
    assert_eq!(text, "r 10.2 mm, ratio 0.50, -15°");
    let mut right = polygon_session(5);
    let text = create_drag(&mut right, pt(100.0, 50.0), pt(110.0, 50.0), false).unwrap();
    assert_eq!(text, "r 10.0 mm, 0°");
}

/// Criterion 4: Ctrl snaps the created angle; A = (100, 50), B = (110, 48)
/// gives a first vertex at (109.85, 47.36) and radius 10.20 mm; without Ctrl
/// the angle is the raw one. `rotation` stays 0 (nothing new is stored).
#[test]
fn ctrl_snaps_the_created_angle() {
    let mut snapped = polygon_session(6);
    create_drag(&mut snapped, pt(100.0, 50.0), pt(110.0, 48.0), true);
    let p = primitive(&snapped);
    let frame = frame_of(p.shape);
    assert!((frame.radius.as_mm() - 10.198).abs() < 0.005);
    assert_eq!(p.rotation.as_radians(), 0.0);
    let vertex = outline_of_rotated(&p.shape, p.rotation)[0].point;
    assert!((vertex.x - 109.85).abs() < 0.01 && (vertex.y - 47.36).abs() < 0.01);
    assert!((orientation_deg(&snapped) + 15.0).abs() < 1e-9);

    let mut free = polygon_session(6);
    create_drag(&mut free, pt(100.0, 50.0), pt(110.0, 48.0), false);
    assert!((orientation_deg(&free) + 11.309_932_474).abs() < 1e-6);
}

/// Criterion 5: with the pointer held still, pressing or releasing Ctrl
/// changes the preview on the next frame; the shape committed is built from
/// the release event's Ctrl state; Shift does nothing; Escape cancels.
#[test]
fn ctrl_is_read_live_and_at_the_release() {
    let mut session = polygon_session(6);
    let (a, b) = (pt(100.0, 50.0), pt(110.0, 48.0));
    session.pointer_hover(a, false, false);
    session.pointer_down(a, false);
    session.pointer_hover(b, false, false);
    let free = session.live_readout().unwrap().text;
    // Ctrl pressed with the pointer still: the host re-runs the hover.
    session.modifiers_changed(false, true);
    session.pointer_hover(b, false, true);
    let snapped = session.live_readout().unwrap().text;
    assert_ne!(free, snapped);
    session.pointer_hover(b, true, false);
    assert_eq!(
        session.live_readout().unwrap().text,
        free,
        "Shift: no effect"
    );
    // Preview was snapped last, the release has no Ctrl: the free shape.
    session.pointer_hover(b, false, true);
    session.pointer_up(b, false, false);
    assert!((orientation_deg(&session) + 11.309_932_474).abs() < 1e-6);

    let mut escaped = polygon_session(6);
    escaped.pointer_hover(a, false, false);
    escaped.pointer_down(a, false);
    escaped.pointer_hover(b, false, true);
    escaped.escape();
    assert!(escaped.live_readout().is_none());
    assert_eq!(
        unpack(99, &escaped.pack("0.1.0").unwrap())
            .unwrap()
            .object_ids()
            .len(),
        0,
        "Escape wrote nothing"
    );
}

/// Criterion 7: a polygon created at 45 degrees and turned by a raw 10 with
/// Ctrl ends at 60; one at 78.7 and a raw 1 ends at 75; the readout during
/// the drag is the shown angle.
#[test]
fn ctrl_rotate_snaps_the_shown_angle() {
    for (created, raw, expected) in [(45.0, 10.0, 60.0), (78.7, 1.0, 75.0), (-30.0, -4.0, -30.0)] {
        let center = pt(100.0, 50.0);
        let mut session = polygon_session(5);
        create_drag(&mut session, center, polar(center, 10.0, created), false);
        let handle = ne_rotate(center, 10.0, created);
        let from = center.vector_to(handle);
        let start = from.y.atan2(from.x).to_degrees();
        let radius = from.length();
        // Far from the pivot so that a small sweep clears the 3 px dead zone.
        let _ = radius;
        let to = polar(center, 200.0, start + raw);
        session.pointer_hover(handle, false, false);
        session.pointer_down(handle, false);
        session.pointer_hover(to, false, true);
        let readout = session.live_readout().expect("a rotate readout").text;
        session.pointer_up(to, false, true);
        let landed = orientation_deg(&session);
        assert!(
            (landed - expected).abs() < 1e-6,
            "created {created}, raw {raw}: landed {landed}"
        );
        let shown: f64 = readout.trim_end_matches('°').parse().unwrap();
        assert!((shown - expected).abs() < 0.06, "readout {readout}");
    }
}

/// Criterion 7: a typed angle becomes the shown angle, 0 puts the first
/// vertex straight right, from any created angle; the typed route lands an
/// edge on an axis for every N of the criterion's table.
#[test]
fn a_typed_angle_is_the_shown_angle_and_puts_edges_on_axes() {
    // (N, the angle at which an edge lies on an axis)
    let table = [
        (3, 0.0),
        (5, 0.0),
        (6, 0.0),
        (7, 0.0),
        (9, 0.0),
        (10, 0.0),
        (4, 45.0),
        (8, 22.5),
        (12, 15.0),
    ];
    let center = pt(100.0, 50.0);
    for (count, axis_angle) in table {
        let mut session = polygon_session(count);
        create_drag(&mut session, center, polar(center, 10.0, 78.7), false);
        let text = format!("{axis_angle}");
        assert_eq!(
            type_angle(&mut session, center, 10.0, &text),
            EntryOutcome::Committed,
            "{count}-gon typed {text}"
        );
        assert!((orientation_deg(&session) - axis_angle).abs() < 1e-9);
        let p = primitive(&session);
        let outline = outline_of_rotated(&p.shape, p.rotation);
        let on_axis = (0..outline.len()).any(|i| {
            let (a, b) = (outline[i].point, outline[(i + 1) % outline.len()].point);
            (a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9
        });
        assert!(on_axis, "{count}-gon at {axis_angle}: no edge on an axis");
    }
    // Typing 0 restores the zero position: the first vertex at centre + (R, 0).
    for count in [3, 6, 12] {
        let mut session = polygon_session(count);
        create_drag(&mut session, center, polar(center, 10.0, -37.3), false);
        // 0 is not the shown angle yet, so it is a real change.
        assert_eq!(
            type_angle(&mut session, center, 10.0, "0"),
            EntryOutcome::Committed
        );
        let p = primitive(&session);
        let vertex = outline_of_rotated(&p.shape, p.rotation)[0].point;
        assert!((vertex.x - 110.0).abs() < 1e-9 && (vertex.y - 50.0).abs() < 1e-9);
    }
    // A square at 0 degrees is a diamond: no edge on an axis.
    let mut diamond = polygon_session(4);
    create_drag(&mut diamond, center, polar(center, 10.0, 0.0), false);
    let p = primitive(&diamond);
    let outline = outline_of_rotated(&p.shape, p.rotation);
    assert!(!(0..4).any(|i| {
        let (a, b) = (outline[i].point, outline[(i + 1) % 4].point);
        (a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9
    }));
}

/// Criterion 1: rotating by a delta turns the shape and its box by the delta
/// and the shown angle by the delta (wrapped), after a typed angle too.
#[test]
fn rotating_changes_the_shown_angle_by_the_delta() {
    let center = pt(100.0, 50.0);
    let mut session = polygon_session(5);
    create_drag(&mut session, center, polar(center, 10.0, 170.0), false);
    let handle = ne_rotate(center, 10.0, 170.0);
    let from = center.vector_to(handle);
    let radius = from.length();
    let start = from.y.atan2(from.x).to_degrees();
    drag(
        &mut session,
        handle,
        polar(center, radius, start + 30.0),
        false,
    );
    assert!((orientation_deg(&session) + 160.0).abs() < 1e-6);
    // The rotation register moved by the delta, the frame angle did not.
    let p = primitive(&session);
    assert!((p.rotation.as_radians().to_degrees() - 30.0).abs() < 1e-6);
    assert!((frame_of(p.shape).angle.as_radians().to_degrees() - 170.0).abs() < 1e-9);
}

/// Criterion 2: a project saved before the change opens with every polygon
/// and star looking as saved and shows the sum of the two registers; the
/// format version did not move; opening and saving rewrites no register.
#[test]
fn an_old_project_opens_unchanged_and_shows_the_orientation() {
    assert_eq!(CURRENT_FORMAT_VERSION, 5);
    let document = Document::new(1);
    let frame = |angle: f64, x: f64| StarFrame {
        center: pt(x, 50.0),
        radius: Length::from_mm(10.0),
        angle: Angle::from_radians(angle.to_radians()),
    };
    // A star at frame angle 78.7, rotation 0; a polygon at 10 and 30.
    let star = document.create_star(
        frame(78.7, 50.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let polygon = document.create_polygon(frame(10.0, 150.0), PointCount::new(6).unwrap());
    let turned = document
        .object(polygon)
        .unwrap()
        .rotated(pt(150.0, 50.0), Angle::from_radians(30.0_f64.to_radians()));
    document.rotate_object(&turned).unwrap();
    let before = [
        document.object(star).unwrap(),
        document.object(polygon).unwrap(),
    ];

    let bytes = pack(&document, "0.1.0").unwrap();
    let mut session = Session::open(2, &bytes).unwrap();
    session.set_tool(Tool::Select);
    let reopened = unpack(99, &session.pack("0.1.0").unwrap()).unwrap();
    assert_eq!(reopened.object(star).unwrap(), before[0]);
    assert_eq!(reopened.object(polygon).unwrap(), before[1]);
    assert!((before[0].orientation().as_radians().to_degrees() - 78.7).abs() < 1e-9);
    assert!((before[1].orientation().as_radians().to_degrees() - 40.0).abs() < 1e-9);

    // The Select tool shows it: a polygon whose frame angle alone is 40
    // opens its angle entry on 40, and so does the star's 78.7.
    for (angle, text) in [(40.0, "40"), (78.7, "78.7")] {
        let document = Document::new(1);
        let _ = document.create_polygon(frame(angle, 250.0), PointCount::new(6).unwrap());
        let mut session = Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap();
        session.set_tool(Tool::Select);
        let ObjectSnapshot::Primitive(shape) = object(&session) else {
            panic!("a primitive");
        };
        let on_outline = outline_of_rotated(&shape.shape, shape.rotation)[0].point;
        session.pointer_hover(on_outline, false, false);
        session.pointer_down(on_outline, false);
        session.pointer_up(on_outline, false, false);
        double_click(&mut session, ne_rotate(pt(250.0, 50.0), 10.0, angle));
        let entry = session.transform_entry().expect("an angle entry");
        assert_eq!(entry.fields[0].prefill, text);
    }
}
