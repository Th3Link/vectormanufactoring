//! Black-box tests for `specs/0005-object-transform/specification.md`
//! criteria 8 and 26-31, the "Scale stroke width" switch, against
//! `Session`'s public API.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_document_core::{
    AnchorId, Document, EllipseFrame, Length, NewAnchor, ObjectSnapshot, Point, RectBounds, Shape,
    pack, unpack,
};
use vecmanf_editor_wasm::{Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn session_of(document: &Document) -> Session {
    Session::open(1, &pack(document, "0.1.0").unwrap()).expect("opens")
}

fn rect_session() -> Session {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(20.0),
    });
    document
        .set_corner_radius(&[id], Length::from_mm(2.0))
        .unwrap();
    session_of(&document)
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

fn only_object(s: &Session) -> ObjectSnapshot {
    let d = unpack(9, &s.pack("0.1.0").unwrap()).unwrap();
    let ids = d.object_ids();
    d.object(ids[0]).unwrap()
}

fn stroke(o: &ObjectSnapshot) -> f64 {
    match o {
        ObjectSnapshot::Primitive(p) => p.stroke_width.as_mm(),
        ObjectSnapshot::Path(p) => p.stroke_width.as_mm(),
    }
}

/// AC 8: with the switch off (the default) a resize by every handle keeps
/// the stroke width, on a rectangle, an ellipse and a path.
#[test]
fn ac8_default_keeps_the_stroke_width_for_every_handle() {
    let handles = [
        pt(40.0, 20.0),
        pt(0.0, 0.0),
        pt(40.0, 0.0),
        pt(0.0, 20.0),
        pt(20.0, 0.0),
        pt(20.0, 20.0),
        pt(0.0, 10.0),
        pt(40.0, 10.0),
    ];
    let docs: Vec<(Document, Point)> = vec![
        {
            let d = Document::new(1);
            d.create_rect(RectBounds {
                origin: pt(0.0, 0.0),
                width: Length::from_mm(40.0),
                height: Length::from_mm(20.0),
            });
            (d, pt(20.0, 0.0))
        },
        {
            let d = Document::new(1);
            d.create_ellipse(EllipseFrame {
                center: pt(20.0, 10.0),
                rx: Length::from_mm(20.0),
                ry: Length::from_mm(10.0),
            });
            (d, pt(20.0, 0.0))
        },
        {
            let d = Document::new(1);
            d.create_path(
                &[
                    NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
                    NewAnchor::corner(AnchorId::new(1, 2), pt(40.0, 20.0)),
                ],
                false,
            );
            (d, pt(20.0, 10.0))
        },
    ];
    for (document, on_outline) in docs {
        for handle in handles {
            let mut s = session_of(&document);
            assert!(!s.scale_stroke_width());
            click(&mut s, on_outline);
            let before = stroke(&only_object(&s));
            drag(&mut s, handle, pt(handle.x + 11.0, handle.y + 7.0));
            let after = only_object(&s);
            assert!((stroke(&after) - before).abs() < 1e-12, "{handle:?}");
        }
    }
}

/// AC 26: switch on: √(sx·sy), and the 0.01 mm floor.
#[test]
fn ac26_switch_on_scales_by_the_geometric_mean_and_floors() {
    let mut s = rect_session();
    s.set_scale_stroke_width(true);
    click(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 10.0), pt(80.0, 10.0)); // sx 2, sy 1
    assert!((stroke(&only_object(&s)) - 0.25 * 2.0_f64.sqrt()).abs() < 1e-9);

    let mut s = rect_session();
    s.set_scale_stroke_width(true);
    click(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 10.0), pt(-80.0, 10.0)); // collapses
    let w = stroke(&only_object(&s));
    assert!((w - 0.01).abs() < 1e-12, "floor, got {w}");
}

/// AC 27: every new session — and every opened project — starts off.
#[test]
fn ac27_every_new_session_starts_off_and_is_never_read_from_the_file() {
    assert!(!Session::new(1).scale_stroke_width());
    let mut s = rect_session();
    s.set_scale_stroke_width(true);
    let bytes = s.pack("0.1.0").unwrap();
    assert!(!Session::open(2, &bytes).unwrap().scale_stroke_width());
}

/// AC 28: the switch's state at the press governs the whole drag.
#[test]
fn ac28_a_mid_drag_toggle_applies_to_the_next_drag() {
    let mut s = rect_session();
    click(&mut s, pt(20.0, 0.0));
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.set_scale_stroke_width(true);
    s.pointer_hover(pt(80.0, 40.0), false, false);
    s.pointer_up(pt(80.0, 40.0), false, false);
    assert!((stroke(&only_object(&s)) - 0.25).abs() < 1e-12, "kept");
    // Next drag (now an 80 x 40 rectangle): the switch is on.
    drag(&mut s, pt(80.0, 40.0), pt(160.0, 80.0));
    assert!((stroke(&only_object(&s)) - 0.5).abs() < 1e-9, "scaled 2x");
}

/// AC 29: toggling writes nothing; saved bytes do not change.
#[test]
fn ac29_toggling_the_switch_does_not_change_the_saved_file() {
    let mut s = rect_session();
    click(&mut s, pt(20.0, 0.0));
    let off = s.pack("0.1.0").unwrap();
    s.set_scale_stroke_width(true);
    let on = s.pack("0.1.0").unwrap();
    assert_eq!(off, on);
    // And the tool is still the Select tool, the object untouched.
    assert_eq!(s.tool(), Tool::Select);
}

/// AC 9, 31: the radius scales the same in both states.
#[test]
fn ac31_the_corner_radius_scales_identically_with_the_switch_either_way() {
    let radius = |on: bool| {
        let mut s = rect_session();
        s.set_scale_stroke_width(on);
        click(&mut s, pt(20.0, 0.0));
        drag(&mut s, pt(40.0, 20.0), pt(80.0, 40.0)); // sx = sy = 2
        let ObjectSnapshot::Primitive(p) = only_object(&s) else {
            panic!("primitive");
        };
        let Shape::Rect { corner_radius, .. } = p.shape else {
            panic!("rect");
        };
        corner_radius.as_mm()
    };
    assert!((radius(false) - 4.0).abs() < 1e-9);
    assert!((radius(true) - 4.0).abs() < 1e-9);
}
