//! Black-box acceptance tests for `specs/0005-object-transform/
//! specification.md`'s document-model criteria (19, 20, 24): a rotated or
//! resized object survives save → close → reopen exactly, and stays the
//! same kind of object.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, ObjectSnapshot, Point,
    PointCount, RectBounds, Shape, StarFrame, Vec2, pack, unpack,
};

fn reopened(document: &Document) -> Document {
    let bytes = pack(document, "0.1.0").expect("pack");
    unpack(7, &bytes).expect("unpack")
}

fn quarter_turn() -> Angle {
    Angle::from_radians(std::f64::consts::FRAC_PI_2)
}

/// AC 19: a rotated primitive keeps its exact rotation and stays the same
/// kind of primitive across a save/reopen — for all four kinds.
#[test]
fn ac19_rotation_round_trips_for_every_primitive_kind_and_none_becomes_a_path() {
    let document = Document::new(1);
    let rect = document.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(4.0),
    });
    let ellipse = document.create_ellipse(EllipseFrame {
        center: Point::new(30.0, 0.0),
        rx: Length::from_mm(5.0),
        ry: Length::from_mm(3.0),
    });
    let star_frame = StarFrame {
        center: Point::new(60.0, 0.0),
        radius: Length::from_mm(8.0),
        angle: Angle::from_radians(0.25),
    };
    let polygon = document.create_polygon(star_frame, PointCount::new(6).unwrap());
    let star = document.create_star(
        star_frame,
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let angles = [0.3, -1.2, 2.0, 3.1];
    for (id, radians) in [rect, ellipse, polygon, star].into_iter().zip(angles) {
        let center = match document.primitive(id).unwrap().shape {
            Shape::Rect { bounds, .. } => Point::new(
                bounds.origin.x + bounds.width.as_mm() / 2.0,
                bounds.origin.y + bounds.height.as_mm() / 2.0,
            ),
            Shape::Ellipse { frame } => frame.center,
            Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => frame.center,
        };
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(center, Angle::from_radians(radians)),
            )
            .expect("rotate");
    }
    let before: Vec<_> = [rect, ellipse, polygon, star]
        .map(|id| document.primitive(id).expect("primitive"))
        .to_vec();

    let after = reopened(&document);
    for (id, expected) in [rect, ellipse, polygon, star].into_iter().zip(before) {
        let got = after.primitive(id).expect("still a primitive, not a path");
        assert_eq!(
            got, expected,
            "rotation and frame exactly as before closing"
        );
        assert!(after.path(id).is_none());
    }
}

/// AC 20, 24: a rotated path stores its baked anchors *and* its
/// `rotation` register, both exactly across a save/reopen.
#[test]
fn ac20_ac24_a_rotated_path_keeps_baked_anchors_and_its_rotation_register() {
    let document = Document::new(1);
    let id = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
            NewAnchor {
                handle_out: Vec2::new(2.0, 1.0),
                ..NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0))
            },
        ],
        false,
    );
    document
        .rotate_object(
            &document
                .object(id)
                .expect("object exists")
                .rotated(Point::new(0.0, 0.0), quarter_turn()),
        )
        .expect("rotate");
    let before = document.path(id).unwrap();
    assert!((before.rotation.as_radians() - quarter_turn().as_radians()).abs() < 1e-12);
    // Baked: (10, 0) turned a quarter about the origin is (0, 10), and the
    // relative handle turned with it.
    assert!(before.anchors[1].point.x.abs() < 1e-9);
    assert!((before.anchors[1].point.y - 10.0).abs() < 1e-9);

    let after = reopened(&document).path(id).expect("path");
    assert_eq!(after, before);
}

/// AC 24: a resize (frame, corner radius and stroke width written
/// together) persists exactly.
#[test]
fn ac24_a_resized_rect_and_path_persist_exactly() {
    let document = Document::new(1);
    let rect = document.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    document
        .resize_rect(
            rect,
            RectBounds {
                origin: Point::new(-2.0, 1.0),
                width: Length::from_mm(15.0),
                height: Length::from_mm(12.5),
            },
            Length::from_mm(3.0),
            Some(Length::from_mm(0.375)),
        )
        .expect("resize");
    let path = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), Point::new(4.0, 0.0)),
        ],
        false,
    );
    document
        .resize_path(
            path,
            &[
                (
                    AnchorId::new(1, 1),
                    Point::new(0.0, 0.0),
                    Vec2::ZERO,
                    Vec2::ZERO,
                ),
                (
                    AnchorId::new(1, 2),
                    Point::new(8.0, 0.0),
                    Vec2::ZERO,
                    Vec2::ZERO,
                ),
            ],
            Some(Length::from_mm(0.5)),
        )
        .expect("resize path");

    let after = reopened(&document);
    assert_eq!(after.object(rect), document.object(rect));
    assert_eq!(after.object(path), document.object(path));
    let ObjectSnapshot::Primitive(p) = after.object(rect).unwrap() else {
        panic!("rect stays a primitive");
    };
    assert!((p.stroke_width.as_mm() - 0.375).abs() < 1e-12);
}

/// Rotation is normalized to (-π, π] when written, so repeated rotate
/// drags never grow an unbounded winding count.
#[test]
fn rotation_is_written_normalized() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(2.0),
        height: Length::from_mm(2.0),
    });
    for _ in 0..5 {
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(Point::new(1.0, 1.0), Angle::from_radians(2.0)),
            )
            .expect("rotate");
    }
    let radians = document.primitive(id).unwrap().rotation.as_radians();
    assert!(radians > -std::f64::consts::PI - 1e-12 && radians <= std::f64::consts::PI + 1e-12);
    // 10 rad ≡ 10 - 4π (≈ -2.566).
    assert!((radians - (10.0 - 4.0 * std::f64::consts::PI)).abs() < 1e-9);
}
