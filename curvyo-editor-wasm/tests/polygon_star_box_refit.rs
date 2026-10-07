//! `specs/polygon-star-box-refit/` criteria 13, 14 and 15 through `Session`'s
//! public API: files saved before the refit open with the same outlines and
//! the box of the shown angle, no gesture writes a stored field it did not
//! write before, and the format is unchanged. The drawn box, hit rule, cursors
//! and drags are tested next to the session code
//! (`src/session/box_refit_tests.rs`), which can read the decoration input.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use curvyo_document_core::{
    Angle, CURRENT_FORMAT_VERSION, Document, InnerRatio, Length, NodeId, ObjectSnapshot, Point,
    PointCount, PrimitiveSnapshot, Shape, StarFrame, outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::oriented_bounds;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn key(session: &mut Session, k: &str) -> KeyOutcome {
    session.key_down(KeyInput {
        key: k,
        ..KeyInput::default()
    })
}

fn fixture(name: &str) -> Vec<u8> {
    let p = format!(
        "{}/../curvyo-document-core/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn frame_of(p: &PrimitiveSnapshot) -> StarFrame {
    match p.shape {
        Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => frame,
        _ => panic!("polygon or star expected"),
    }
}

/// A file as a build from before the refit wrote it: a polygon with frame
/// angle 78.7 and rotation 0, a star with frame angle 10 and rotation 30.
fn old_file() -> (Vec<u8>, NodeId, NodeId) {
    let d = Document::new(1);
    let frame = |cx: f64, deg: f64| StarFrame {
        center: pt(cx, 50.0),
        radius: Length::from_mm(20.0),
        angle: Angle::from_radians(deg.to_radians()),
    };
    let polygon = d.create_polygon(frame(50.0, 78.7), PointCount::new(5).unwrap());
    let star = d.create_star(
        frame(200.0, 10.0),
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let o = d.object(star).unwrap();
    d.rotate_object(&o.rotated(pt(200.0, 50.0), Angle::from_radians(30.0_f64.to_radians())))
        .unwrap();
    (pack(&d, "0.1.0").unwrap(), polygon, star)
}

fn change_count(session: &Session) -> usize {
    let doc = unpack(99, &session.pack("0.1.0").unwrap()).unwrap();
    let l = loro::LoroDoc::new();
    l.import(&doc.export_loro_snapshot().unwrap()).unwrap();
    l.len_changes()
}

fn primitive(session: &Session, id: NodeId) -> PrimitiveSnapshot {
    let doc = unpack(99, &session.pack("0.1.0").unwrap()).unwrap();
    doc.primitive(id).unwrap()
}

fn select_first_vertex(session: &mut Session, id: NodeId) {
    let p = primitive(session, id);
    let v = outline_of_rotated(&p.shape, p.rotation)[0].point;
    session.pointer_hover(v, false, false);
    session.pointer_down(v, false);
    session.pointer_up(v, false, false);
    assert_eq!(session.selected_object_count(), 1);
}

fn box_degrees(session: &Session, id: NodeId) -> f64 {
    let doc = unpack(99, &session.pack("0.1.0").unwrap()).unwrap();
    oriented_bounds(&doc.object(id).unwrap())
        .angle
        .as_radians()
        .to_degrees()
}

fn wrap(d: f64) -> f64 {
    Angle::from_radians(d.to_radians())
        .normalized()
        .as_radians()
        .to_degrees()
}

#[test]
fn the_format_version_is_unchanged() {
    assert_eq!(CURRENT_FORMAT_VERSION, 5);
}

/// Criterion 13: old outlines, shown angles 78.7 and 40, boxes in those
/// directions, and opening, selecting and hovering write nothing.
#[test]
fn an_old_file_opens_with_the_same_outlines_and_the_box_of_its_shown_angle() {
    let (bytes, polygon, star) = old_file();
    let reference = unpack(7, &bytes).unwrap();
    let mut session = Session::open(3, &bytes).unwrap();
    session.set_tool(Tool::Select);
    let before = change_count(&session);
    for (id, shown) in [(polygon, 78.7), (star, 40.0)] {
        let was = reference.primitive(id).unwrap();
        select_first_vertex(&mut session, id);
        assert!(
            (wrap(box_degrees(&session, id) - shown)).abs() < 1e-9,
            "{shown}"
        );
        assert_eq!(key(&mut session, "r"), KeyOutcome::EntryOpened);
        let prefill = session.transform_entry().unwrap().fields[0].prefill.clone();
        assert!(
            (prefill.parse::<f64>().unwrap() - shown).abs() <= 0.051,
            "{prefill}"
        );
        session.cancel_transform_entry();
        // Hovering writes nothing either.
        session.pointer_hover(pt(900.0, 900.0), false, false);
        assert_eq!(primitive(&session, id), was);
    }
    assert_eq!(change_count(&session), before, "looking writes nothing");
    let saved = unpack(8, &session.pack("0.1.0").unwrap()).unwrap();
    for id in reference.object_ids() {
        assert_eq!(saved.object(id), reference.object(id));
    }
}

/// Criterion 13, 14: the shipped fixtures still open with every object equal.
#[test]
fn the_shipped_fixtures_round_trip_unchanged() {
    for name in ["primitives_v3.curvyo", "rotation_v5.curvyo"] {
        let bytes = fixture(name);
        let reference = unpack(7, &bytes).unwrap();
        let mut session = Session::open(3, &bytes).unwrap();
        session.set_tool(Tool::Select);
        let saved = unpack(8, &session.pack("0.1.0").unwrap()).unwrap();
        for id in reference.object_ids() {
            assert_eq!(saved.object(id), reference.object(id), "{name}");
            if let Some(ObjectSnapshot::Primitive(p)) = reference.object(id)
                && matches!(p.shape, Shape::Polygon { .. } | Shape::Star { .. })
            {
                let shown = reference.object(id).unwrap().orientation().as_radians();
                let boxed = oriented_bounds(&saved.object(id).unwrap())
                    .angle
                    .as_radians();
                assert!(
                    Angle::from_radians(shown - boxed)
                        .normalized()
                        .as_radians()
                        .abs()
                        < 1e-12,
                    "{name}"
                );
            }
        }
    }
}

/// Criteria 13, 14: a typed rotation about the centre writes `rotation` only,
/// a typed radius writes the frame radius only; the frame angle is bit-identical.
#[test]
fn a_rotation_writes_the_rotation_and_a_resize_the_radius_never_the_frame_angle() {
    let (bytes, polygon, star) = old_file();
    for id in [polygon, star] {
        let mut session = Session::open(3, &bytes).unwrap();
        session.set_tool(Tool::Select);
        select_first_vertex(&mut session, id);
        let start = primitive(&session, id);

        assert_eq!(key(&mut session, "r"), KeyOutcome::EntryOpened);
        session.commit_transform_entry("0", "", 0);
        let turned = primitive(&session, id);
        assert_eq!(
            frame_of(&turned).angle.as_radians().to_bits(),
            frame_of(&start).angle.as_radians().to_bits()
        );
        assert_eq!(frame_of(&turned).center, frame_of(&start).center);
        assert_eq!(frame_of(&turned).radius, frame_of(&start).radius);
        assert_ne!(turned.rotation, start.rotation);
        assert!(box_degrees(&session, id).abs() < 1e-9, "upright after R, 0");

        select_first_vertex(&mut session, id);
        assert_eq!(key(&mut session, "s"), KeyOutcome::EntryOpened);
        assert_eq!(session.transform_entry().unwrap().kind, "radius");
        session.commit_transform_entry("30", "", 0);
        let grown = primitive(&session, id);
        assert_eq!(grown.rotation, turned.rotation);
        assert_eq!(
            frame_of(&grown).angle.as_radians().to_bits(),
            frame_of(&start).angle.as_radians().to_bits()
        );
        assert_eq!(frame_of(&grown).center, frame_of(&start).center);
        assert!((frame_of(&grown).radius.as_mm() - 30.0).abs() < 1e-9);
        assert!(
            box_degrees(&session, id).abs() < 1e-9,
            "the box keeps its direction"
        );
    }
}
