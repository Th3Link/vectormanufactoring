//! The box a gradient spans, as the session hands it to the renderer
//! (`specs/0007-stroke-and-fill-styling` criteria 21, 22; the tester cases of
//! the architect's note): it is the object's oriented selection box, so the
//! gradient turns with a rotation and re-fits after a skew or a conversion.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    AnchorId, Angle, Document, FillMode, FillModeTarget, GradientStop, Length, NewAnchor, NodeId,
    Point, PointCount, RectBounds, StarFrame, StopId, Vec2, pack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::GradientFill;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn linear(document: &Document, id: NodeId) {
    document
        .set_fill_mode(
            FillMode::Linear,
            &[FillModeTarget {
                id,
                seed_stops: GradientStop::default_pair(
                    curvyo_document_core::Color { r: 255, g: 0, b: 0 },
                    StopId::new(1, 1),
                    StopId::new(1, 2),
                )
                .to_vec(),
            }],
        )
        .unwrap();
}

fn session_of(document: &Document) -> Session {
    let mut session = Session::open(5, &pack(document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session
}

fn only_fill(session: &Session) -> GradientFill {
    let list = session.draw_list();
    assert_eq!(list.gradients().len(), 1);
    list.gradients()[0].clone()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

/// Rotated 90 degrees, the ramp runs top to bottom on screen.
#[test]
fn a_rectangle_rotated_a_quarter_turn_has_a_top_to_bottom_ramp() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(20.0),
        height: Length::from_mm(10.0),
    });
    linear(&document, id);
    document
        .rotate_object(&document.object(id).unwrap().rotated(
            pt(10.0, 5.0),
            Angle::from_radians(std::f64::consts::FRAC_PI_2),
        ))
        .unwrap();
    let fill = only_fill(&session_of(&document));
    // The rotated rectangle spans x 5..15 and y -5..15 on screen.
    assert!(
        near(fill.coordinate(pt(10.0, -5.0))[0], 0.0),
        "top is the start"
    );
    assert!(near(fill.coordinate(pt(10.0, 5.0))[0], 0.5));
    assert!(
        near(fill.coordinate(pt(10.0, 15.0))[0], 1.0),
        "bottom is the end"
    );
    assert!(
        near(fill.coordinate(pt(5.0, 5.0))[0], 0.5),
        "constant across"
    );
}

/// A path skewed 30 degrees re-fits its ramp to its new bounds.
#[test]
fn a_skewed_path_refits_its_gradient_box() {
    let document = Document::new(1);
    let anchors = [(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (0.0, 10.0)]
        .iter()
        .enumerate()
        .map(|(n, (x, y))| NewAnchor::corner(AnchorId::new(1, n as u64 + 1), pt(*x, *y)))
        .collect::<Vec<_>>();
    let id = document.create_path(&anchors, true);
    linear(&document, id);
    let before = only_fill(&session_of(&document));
    assert_eq!((before.frame.min.x, before.frame.max.x), (0.0, 20.0));
    // x' = x + y * tan(30 degrees)
    let shear = 30.0_f64.to_radians().tan();
    let skewed: Vec<_> = anchors
        .iter()
        .map(|a| {
            (
                a.id,
                pt(a.point.x + a.point.y * shear, a.point.y),
                Vec2::ZERO,
                Vec2::ZERO,
            )
        })
        .collect();
    document.resize_path(id, &skewed, None).unwrap();
    let after = only_fill(&session_of(&document));
    assert!((after.frame.min.x - 0.0).abs() < 1e-9);
    assert!((after.frame.max.x - (20.0 + 10.0 * shear)).abs() < 1e-9);
    assert!(near(
        after.coordinate(pt(20.0 + 10.0 * shear, 10.0))[0],
        1.0
    ));
}

/// "Object to path" turns a triangle's square box into its tight box, so the
/// ramp re-fits at the conversion (criterion 21's stated limit).
#[test]
fn converting_a_triangle_to_a_path_tightens_the_gradient_box() {
    let document = Document::new(1);
    let id = document.create_polygon(
        StarFrame {
            center: pt(50.0, 50.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(3).unwrap(),
    );
    linear(&document, id);
    let mut session = session_of(&document);
    let before = only_fill(&session);
    assert!(
        (before.frame.max.x - before.frame.min.x - 20.0).abs() < 1e-6,
        "the square"
    );
    // Select it by its left edge, then convert.
    session.pointer_hover(pt(45.0, 50.0), false, false);
    session.pointer_down(pt(45.0, 50.0), false);
    session.pointer_up(pt(45.0, 50.0), false, false);
    session.convert_selected_to_paths();
    let after = only_fill(&session);
    let width = after.frame.max.x - after.frame.min.x;
    assert!((width - 15.0).abs() < 1e-6, "tight: 1.5 R, got {width}");
}
