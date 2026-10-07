//! Tester acceptance tests for the double-click dispatch of
//! `specs/object-transform-refinements` (criteria 3, 18, 22, 23, 49) and of
//! slice 4 (22, 23), driven the way the browser host drives `Session`: the
//! first press and release reach the session, the second press is withheld,
//! and its release becomes one `double_click` at the second press's position
//! and modifiers, followed by a hover. Everything goes through the public API.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::similar_names)]

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, Point, PointCount,
    RectBounds, StarFrame, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn open(d: &Document) -> Session {
    let bytes = pack(d, "0.1.0").unwrap();
    let mut s = Session::open(2, &bytes).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn bytes(s: &Session) -> Vec<u8> {
    s.pack("0.1.0").unwrap()
}

fn rect_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_rect(RectBounds {
        origin: pt(10.0, 20.0),
        width: Length::from_mm(120.0),
        height: Length::from_mm(80.0),
    });
    d
}

fn ellipse_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(70.0, 60.0),
        rx: Length::from_mm(60.0),
        ry: Length::from_mm(40.0),
    });
    d
}

fn frame() -> StarFrame {
    StarFrame {
        center: pt(70.0, 60.0),
        radius: Length::from_mm(40.0),
        angle: Angle::from_radians(-std::f64::consts::FRAC_PI_2),
    }
}

fn polygon_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_polygon(frame(), PointCount::new(5).unwrap());
    d
}

fn star_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_star(
        frame(),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    d
}

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

/// Closed triangle (0,0) (120,0) (120,80): box (0,0)-(120,80).
fn path_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 0.0, 0.0),
            anchor(2, 120.0, 0.0),
            anchor(3, 120.0, 80.0),
        ],
        true,
    );
    d
}

/// The browser's double-click: press and release at `first`, the second press
/// at `second` withheld, `double_click` at it, then the re-hover.
fn dbl(s: &mut Session, first: Point, second: Point, shift: bool, ctrl: bool) -> bool {
    s.pointer_hover(first, shift, ctrl);
    s.pointer_down(first, shift);
    s.pointer_up(first, shift, ctrl);
    s.pointer_hover(second, shift, ctrl);
    let hint = s.double_click(second, shift, ctrl);
    s.pointer_hover(second, shift, ctrl);
    hint
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn px(s: &Session, pixels: f64) -> f64 {
    pixels / s.view().scale()
}

/// A press and release at `p` with the object not selected.
fn unselected_handoff(doc: &Document, at: Point, want: Tool, label: &str) {
    let mut s = open(doc);
    let before = bytes(&s);
    let hint = dbl(&mut s, at, at, false, false);
    // `unified-object-editing` criteria 31, 32: a path hands off to the Node
    // tool; a primitive changes no tool and asks for the edit hint.
    assert_eq!(s.tool(), want, "{label}: handoff at {at:?}");
    assert_eq!(hint, want == Tool::Select, "{label}: edit hint at {at:?}");
    assert!(s.transform_entry().is_none(), "{label}: no entry");
    assert_eq!(bytes(&s), before, "{label}: nothing written");
}

#[test]
fn double_click_on_the_outline_of_an_unselected_object_hands_off_a_path_and_only_hints_for_a_primitive_at_every_handle_spot()
 {
    // Rectangle: four corners and four edge midpoints (resize handle spots).
    let r = rect_doc();
    for at in [
        pt(10.0, 20.0),
        pt(130.0, 20.0),
        pt(130.0, 100.0),
        pt(10.0, 100.0),
        pt(70.0, 20.0),
        pt(130.0, 60.0),
        pt(70.0, 100.0),
        pt(10.0, 60.0),
    ] {
        unselected_handoff(&r, at, Tool::Select, "rect");
    }
    // Ellipse: the four extreme points are its edge handle spots.
    let e = ellipse_doc();
    for at in [
        pt(70.0, 20.0),
        pt(130.0, 60.0),
        pt(70.0, 100.0),
        pt(10.0, 60.0),
    ] {
        unselected_handoff(&e, at, Tool::Select, "ellipse");
    }
    // Polygon and star: the top vertex is on the outline and at the N spot.
    unselected_handoff(&polygon_doc(), pt(70.0, 20.0), Tool::Select, "polygon");
    unselected_handoff(&star_doc(), pt(70.0, 20.0), Tool::Select, "star");
    // Path: the top-edge midpoint and an anchor.
    let p = path_doc();
    for at in [
        pt(60.0, 0.0),
        pt(0.0, 0.0),
        pt(120.0, 80.0),
        pt(120.0, 40.0),
    ] {
        unselected_handoff(&p, at, Tool::Node, "path");
    }
}

#[test]
fn a_second_press_a_few_pixels_off_still_only_hints_for_an_unselected_primitive() {
    let mut s = open(&rect_doc());
    let first = pt(70.0, 20.0);
    let second = pt(70.0 + px(&s, 4.0), 20.0 + px(&s, 2.0));
    assert!(dbl(&mut s, first, second, false, false), "the edit hint");
    assert_eq!(s.tool(), Tool::Select, "no handoff");
    assert!(s.transform_entry().is_none());
}

#[test]
fn click_then_double_click_on_the_outline_of_a_selected_object_at_a_handle_spot_opens_the_entry() {
    // The handle exists after the click: the double-click grabs it twice.
    let mut s = open(&rect_doc());
    let n = pt(70.0, 20.0);
    click(&mut s, n);
    let before = bytes(&s);
    dbl(&mut s, n, n, false, false);
    assert_eq!(s.tool(), Tool::Select, "no handoff");
    let view = s.transform_entry().expect("size entry on the N handle");
    assert_eq!(view.kind, "size");
    assert_eq!(bytes(&s), before);
}

#[test]
fn a_double_click_on_each_handle_of_a_selected_rectangle_opens_the_right_entry() {
    let k = 1.0;
    let _ = k;
    let mut probe = open(&rect_doc());
    click(&mut probe, pt(70.0, 20.0));
    let rot = |sx: f64, sy: f64, pxs: &Session| {
        // 32 px out on the diagonal of the corner.
        let d = 32.0 / std::f64::consts::SQRT_2 / pxs.view().scale();
        pt(
            if sx > 0.0 { 130.0 + d } else { 10.0 - d },
            if sy > 0.0 { 100.0 + d } else { 20.0 - d },
        )
    };
    let resize = [
        pt(10.0, 20.0),
        pt(130.0, 20.0),
        pt(130.0, 100.0),
        pt(10.0, 100.0),
        pt(70.0, 20.0),
        pt(130.0, 60.0),
        pt(70.0, 100.0),
        pt(10.0, 60.0),
    ];
    for at in resize {
        let mut s = open(&rect_doc());
        click(&mut s, pt(70.0, 20.0));
        dbl(&mut s, at, at, false, false);
        assert_eq!(s.tool(), Tool::Select, "resize {at:?}: no handoff");
        assert_eq!(
            s.transform_entry().map(|e| e.kind),
            Some("size"),
            "resize {at:?}"
        );
    }
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let mut s = open(&rect_doc());
        click(&mut s, pt(70.0, 20.0));
        let at = rot(sx, sy, &s);
        dbl(&mut s, at, at, false, false);
        assert_eq!(s.tool(), Tool::Select, "rotate: no handoff");
        assert_eq!(s.transform_entry().map(|e| e.kind), Some("angle"));
    }
}

#[test]
fn a_selected_handle_double_click_with_the_second_press_a_few_pixels_off_the_first_on_the_same_handle()
 {
    let mut s = open(&rect_doc());
    click(&mut s, pt(70.0, 20.0));
    let h = pt(130.0, 100.0);
    let second = pt(130.0 + px(&s, 3.0), 100.0 + px(&s, 3.0));
    dbl(&mut s, h, second, false, false);
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.transform_entry().map(|e| e.kind), Some("size"));
}

#[test]
fn a_double_click_on_the_centre_handle_and_inside_the_box_still_hands_off() {
    for at in [pt(70.0, 60.0), pt(40.0, 40.0), pt(100.0, 80.0)] {
        let mut s = open(&rect_doc());
        click(&mut s, pt(70.0, 20.0));
        dbl(&mut s, at, at, false, false);
        assert_eq!(s.tool(), Tool::Select, "{at:?}");
        assert!(s.transform_entry().is_none());
    }
}

#[test]
fn first_press_on_the_body_second_on_a_handle_does_not_open_an_entry() {
    // The first press lands inside the box (a body grab), the second within
    // 5 px but on the Se resize handle: the handle was not grabbed twice.
    let mut s = open(&rect_doc());
    click(&mut s, pt(70.0, 20.0));
    let at = |inward_px: f64, s: &Session| {
        let d = px(s, inward_px);
        pt(130.0 - d, 100.0 - d)
    };
    // Resize handle radius is min(16, s/3) = 16 px; 20 px inward is outside it.
    let first = at(20.0, &s);
    let second = at(16.0, &s);
    dbl(&mut s, first, second, false, false);
    assert!(s.transform_entry().is_none(), "no entry");
}

#[test]
fn two_slow_clicks_on_a_handle_open_nothing_and_write_nothing() {
    let mut s = open(&rect_doc());
    click(&mut s, pt(70.0, 20.0));
    let before = bytes(&s);
    let h = pt(130.0, 100.0);
    click(&mut s, h);
    click(&mut s, h);
    assert!(s.transform_entry().is_none());
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(bytes(&s), before);
}

#[test]
fn a_press_elsewhere_clears_the_remembered_handle() {
    // A press on the Se handle, then a slow press far away on empty canvas,
    // then a withheld pair that starts on the body: no stale handle.
    let mut s = open(&rect_doc());
    click(&mut s, pt(70.0, 20.0));
    click(&mut s, pt(130.0, 100.0));
    click(&mut s, pt(300.0, 300.0)); // deselects
    click(&mut s, pt(70.0, 20.0)); // reselects
    // First press of the pair on the outline spot of the N handle region of an
    // object that is selected: pair on the same handle opens the entry (that is
    // the contract); a pair whose first press misses everything does not.
    s.set_tool(Tool::Select);
    click(&mut s, pt(300.0, 300.0));
    assert!(s.transform_entry().is_none());
    dbl(&mut s, pt(300.0, 300.0), pt(130.0, 100.0), false, false);
    assert!(
        s.transform_entry().is_none(),
        "first press missed everything"
    );
}

#[test]
fn double_click_on_a_skew_handle_opens_the_skew_entry_without_handoff_for_a_path() {
    let mut s = open(&path_doc());
    click(&mut s, pt(60.0, 0.0));
    let before = bytes(&s);
    let out = px(&s, 16.0);
    for at in [
        pt(60.0, -out),
        pt(120.0 + out, 40.0),
        pt(60.0, 80.0 + out),
        pt(-out, 40.0),
    ] {
        s.pointer_hover(at, false, false);
        assert!(
            s.cursor_hint().contains("skew"),
            "{at:?}: cursor {}",
            s.cursor_hint()
        );
        dbl(&mut s, at, at, false, false);
        assert_eq!(s.tool(), Tool::Select, "{at:?}: no handoff");
        assert_eq!(
            s.transform_entry().map(|entry| entry.kind),
            Some("skew"),
            "{at:?}: the skew entry opens (`edit-interaction-polish` criterion 9)"
        );
        assert_eq!(bytes(&s), before, "{at:?}: nothing written");
        s.cancel_transform_entry();
        // The cursor still describes the skew handle after the double-click.
        assert!(
            s.cursor_hint().contains("skew"),
            "{at:?}: cursor after double-click: {}",
            s.cursor_hint()
        );
    }
}

#[test]
fn rapid_triple_click_on_a_handle_keeps_one_entry_and_the_select_tool() {
    let mut s = open(&rect_doc());
    click(&mut s, pt(70.0, 20.0));
    let h = pt(130.0, 100.0);
    dbl(&mut s, h, h, false, false);
    assert!(s.transform_entry().is_some());
    // Third press, also withheld by the host (within 400 ms of the second).
    s.double_click(h, false, false);
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.transform_entry().map(|e| e.kind), Some("size"));
}

#[test]
fn handle_double_click_then_a_normal_press_elsewhere_closes_the_entry_and_the_next_pair_only_hints()
{
    let mut s = open(&rect_doc());
    click(&mut s, pt(70.0, 20.0));
    dbl(&mut s, pt(130.0, 100.0), pt(130.0, 100.0), false, false);
    assert!(s.transform_entry().is_some());
    click(&mut s, pt(70.0, 60.0));
    assert!(s.transform_entry().is_none());
    // Inside the box, away from the centre handle (which opens the typed move
    // since `edit-interaction-polish` PR 3): only the hint.
    assert!(dbl(&mut s, pt(40.0, 50.0), pt(40.0, 50.0), false, false));
    assert_eq!(s.tool(), Tool::Select);
}

#[test]
fn a_stale_handle_from_an_earlier_press_does_not_survive_a_tool_round_trip() {
    // Press on the Se handle in Select, switch tools and back, then a pair
    // whose first press never reached the Select tool (it was withheld).
    let mut s = open(&rect_doc());
    click(&mut s, pt(70.0, 20.0));
    click(&mut s, pt(130.0, 100.0));
    s.set_tool(Tool::Rectangle);
    s.set_tool(Tool::Select);
    s.double_click(pt(130.0, 100.0), false, false);
    assert!(
        s.transform_entry().is_none(),
        "a double-click whose first press the Select tool never saw opened an entry"
    );
}

#[test]
fn a_drag_then_a_press_at_the_old_spot_does_not_open_an_entry() {
    // Press the Se handle, drag it away (a resize commits), then the host sees
    // another press within 400 ms and 5 px of the first press position.
    let mut s = open(&rect_doc());
    click(&mut s, pt(70.0, 20.0));
    let h = pt(130.0, 100.0);
    s.pointer_hover(h, false, false);
    s.pointer_down(h, false);
    s.pointer_hover(pt(150.0, 120.0), false, false);
    s.pointer_up(pt(150.0, 120.0), false, false);
    s.double_click(h, false, false);
    assert!(
        s.transform_entry().is_none(),
        "entry opened after a completed drag"
    );
}

/// `unified-object-editing` criterion 31 ("as before"): double-clicking a
/// polygon that was converted to a path, on a segment, hands off to the Node
/// tool from the Select tool and writes nothing: the hand-off does not insert a node, neither on
/// the first press nor on the withheld second one. The same holds on
/// `origin/main`, where `select_double_click` only switched the tool.
#[test]
fn double_click_on_a_converted_polygons_segment_hands_off_without_inserting_a_node() {
    let mut s = open(&polygon_doc());
    // Select the polygon at its top vertex, then convert it.
    click(&mut s, pt(70.0, 20.0));
    s.convert_selected_to_paths();
    let after_conversion = bytes(&s);
    let nodes = |s: &Session| {
        let d = unpack(3, &bytes(s)).unwrap();
        d.object_ids()
            .iter()
            .filter_map(|id| d.object(*id))
            .map(|o| match o {
                curvyo_document_core::ObjectSnapshot::Path(p) => p.anchors.len(),
                curvyo_document_core::ObjectSnapshot::Primitive(_) => 0,
            })
            .sum::<usize>()
    };
    assert_eq!(nodes(&s), 5, "a five-point polygon becomes five anchors");

    // The midpoint of the segment between the first two anchors.
    let d = unpack(3, &after_conversion).unwrap();
    let curvyo_document_core::ObjectSnapshot::Path(path) = d.object(d.object_ids()[0]).unwrap()
    else {
        panic!("converted to a path");
    };
    let (a, b) = (path.anchors[0].point, path.anchors[1].point);
    let mid = pt(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y));

    // The conversion leaves the Node tool active; back in the Select tool the
    // double-click is the hand-off under test.
    assert_eq!(s.tool(), Tool::Node, "conversion activates the Node tool");
    s.set_tool(Tool::Select);
    let hint = dbl(&mut s, mid, mid, false, false);
    assert!(!hint, "a path gives no edit hint");
    assert_eq!(s.tool(), Tool::Node, "the Node tool is active");
    assert_eq!(nodes(&s), 5, "no node was inserted");
    assert_eq!(bytes(&s), after_conversion, "nothing was written");
}
