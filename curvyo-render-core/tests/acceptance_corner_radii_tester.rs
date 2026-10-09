//! Independent test for `specs/0013-rectangle-corner-radii/` PART 1, "render
//! unchanged for equal radii": the stroke of a rectangle with four equal radii
//! is vertex-for-vertex the stroke of a closed path built from the single-radius
//! outline of `origin/main` (copied below as the oracle).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    missing_docs
)]

use curvyo_document_core::{
    AnchorId, AnchorKind, CornerRadii, Document, Length, NewAnchor, ObjectSnapshot, Point,
    RectBounds, Vec2, ViewTransform,
};
use curvyo_render_core::build_artwork;

fn old_outline(x: f64, y: f64, w: f64, h: f64, radius: f64) -> Vec<(Point, Vec2, Vec2)> {
    let r = radius.clamp(0.0, (w.min(h) / 2.0).max(0.0));
    let z = Vec2::ZERO;
    let p = Point::new;
    if r <= 0.0 {
        return vec![
            (p(x, y), z, z),
            (p(x + w, y), z, z),
            (p(x + w, y + h), z, z),
            (p(x, y + h), z, z),
        ];
    }
    let k = curvyo_document_core::KAPPA * r;
    vec![
        (p(x + r, y), Vec2::new(-k, 0.0), z),
        (p(x + w - r, y), z, Vec2::new(k, 0.0)),
        (p(x + w, y + r), Vec2::new(0.0, -k), z),
        (p(x + w, y + h - r), z, Vec2::new(0.0, k)),
        (p(x + w - r, y + h), Vec2::new(k, 0.0), z),
        (p(x + r, y + h), z, Vec2::new(-k, 0.0)),
        (p(x, y + h - r), Vec2::new(0.0, k), z),
        (p(x, y + r), z, Vec2::new(0.0, -k)),
    ]
}

#[test]
fn equal_radii_render_exactly_like_the_old_outline() {
    let mut n = 0;
    for (w, h) in [(40.0, 20.0), (10.0, 10.0), (100.0, 33.3), (7.0, 90.0)] {
        for r in [0.0, 0.5, 3.0, 9.99, 10.0, 50.0] {
            for scale in [0.05, 1.0, 3.78, 40.0] {
                let view = ViewTransform::new(scale, Point::new(0.0, 0.0));
                let rect_doc = Document::new(1);
                let id = rect_doc.create_rect(RectBounds {
                    origin: Point::new(5.0, -3.0),
                    width: Length::from_mm(w),
                    height: Length::from_mm(h),
                });
                rect_doc
                    .set_corner_radii(&[(id, CornerRadii::uniform(Length::from_mm(r)))])
                    .unwrap();
                let prim = rect_doc.primitive(id).unwrap();
                let a = build_artwork(&[ObjectSnapshot::Primitive(prim.clone())], &[], view);

                let path_doc = Document::new(2);
                let anchors: Vec<NewAnchor> = old_outline(5.0, -3.0, w, h, r)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (point, handle_in, handle_out))| NewAnchor {
                        id: AnchorId::new(2, i as u64 + 1),
                        point,
                        handle_in,
                        handle_out,
                        kind: AnchorKind::Corner,
                    })
                    .collect();
                let pid = path_doc.create_path(&anchors, true);
                let path = path_doc.path(pid).unwrap();
                assert_eq!(path.style.stroke.width, prim.style.stroke.width);
                let b = build_artwork(&[ObjectSnapshot::Path(path)], &[], view);
                assert_eq!(
                    format!("{:?}", a.triangles),
                    format!("{:?}", b.triangles),
                    "w {w} h {h} r {r} scale {scale}"
                );
                n += 1;
            }
        }
    }
    assert!(n > 90);
}

#[test]
fn unequal_radii_draw_something_different_and_finite() {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(20.0),
    });
    let view = ViewTransform::new(3.78, Point::new(0.0, 0.0));
    let sharp = build_artwork(
        &[ObjectSnapshot::Primitive(d.primitive(id).unwrap())],
        &[],
        view,
    );
    d.set_corner_radii(&[(
        id,
        CornerRadii {
            tl: Length::from_mm(8.0),
            tr: Length::from_mm(0.0),
            br: Length::from_mm(3.0),
            bl: Length::from_mm(0.0),
        },
    )])
    .unwrap();
    let mixed = build_artwork(
        &[ObjectSnapshot::Primitive(d.primitive(id).unwrap())],
        &[],
        view,
    );
    assert_ne!(
        format!("{:?}", sharp.triangles),
        format!("{:?}", mixed.triangles)
    );
    assert!(mixed.triangles.len() > sharp.triangles.len());
    assert!(
        mixed
            .triangles
            .iter()
            .all(|v| format!("{v:?}").find("NaN").is_none())
    );
}
