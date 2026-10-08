//! The press order with fills (`specs/0007-stroke-and-fill-styling`, criteria
//! 23, 27 to 29): a filled interior takes a press, a Shift press over a filled
//! shape toggles it, and a plain press inside the selected box goes to a
//! filled object lying above the selected one.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    Document, EllipseFrame, FillMode, FillModeTarget, Length, NodeId, ObjectSnapshot, Point,
    RectBounds, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, Modifiers, ObjectSelection, PressTarget, SelectPointerDownOutcome, SelectTool,
    TransformHandleTolerances, classify_press,
};

const SCALE: f64 = 4.0;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn tolerance() -> Tolerance {
    Tolerance::from_mm(1.0)
}

fn tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(SCALE)
}

fn square(document: &Document, x: f64, y: f64, size: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(size),
        height: Length::from_mm(size),
    })
}

fn circle(document: &Document, x: f64, y: f64, r: f64) -> NodeId {
    document.create_ellipse(EllipseFrame {
        center: pt(x, y),
        rx: Length::from_mm(r),
        ry: Length::from_mm(r),
    })
}

fn fill(document: &Document, id: NodeId) {
    document
        .set_fill_mode(
            FillMode::Solid,
            &[FillModeTarget {
                id,
                seed_stops: vec![],
            }],
        )
        .unwrap();
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn selected(ids: &[NodeId]) -> ObjectSelection {
    let mut selection = ObjectSelection::new();
    for (n, id) in ids.iter().enumerate() {
        if n == 0 {
            selection.select_single(*id);
        } else {
            selection.toggle(*id);
        }
    }
    selection
}

fn press(
    document: &Document,
    selection: &ObjectSelection,
    point: Point,
    shift: bool,
) -> PressTarget {
    classify_press(
        &objects(document),
        selection,
        point,
        tolerance(),
        tolerances(),
        shift,
    )
}

#[test]
fn ac28_a_drag_that_starts_inside_an_unselected_filled_shape_moves_it_not_a_marquee() {
    let document = Document::new(1);
    let id = square(&document, 0.0, 0.0, 40.0);
    fill(&document, id);
    let none = ObjectSelection::new();
    assert_eq!(
        press(&document, &none, pt(20.0, 20.0), false),
        PressTarget::Object(id)
    );

    let objs = objects(&document);
    let mut tool = SelectTool::new();
    let mut selection = ObjectSelection::new();
    let outcome = tool.pointer_down(
        &objs,
        &mut selection,
        pt(20.0, 20.0),
        tolerance(),
        tolerances(),
        false,
    );
    assert_eq!(outcome, SelectPointerDownOutcome::Selected);
    assert_eq!(selection.ids(), [id]);
    let live = tool
        .live_move(pt(30.0, 25.0), false, false)
        .expect("a move drag");
    assert_eq!(live.offset.x, 10.0);
}

#[test]
fn ac28_an_unfilled_shape_still_starts_a_marquee_from_its_interior() {
    let document = Document::new(1);
    let _hollow = square(&document, 0.0, 0.0, 40.0);
    let none = ObjectSelection::new();
    assert_eq!(
        press(&document, &none, pt(20.0, 20.0), false),
        PressTarget::Empty
    );
}

#[test]
fn ac28_a_shift_press_over_a_filled_shape_toggles_it_into_the_selection() {
    let document = Document::new(1);
    let first = square(&document, 0.0, 0.0, 40.0);
    let second = square(&document, 100.0, 0.0, 40.0);
    fill(&document, second);
    let selection = selected(&[first]);
    assert_eq!(
        press(&document, &selection, pt(120.0, 20.0), true),
        PressTarget::Object(second)
    );
    let objs = objects(&document);
    let mut tool = SelectTool::new();
    let mut selection = selected(&[first]);
    tool.pointer_down(
        &objs,
        &mut selection,
        pt(120.0, 20.0),
        tolerance(),
        tolerances(),
        true,
    );
    tool.pointer_up(
        &document,
        &objs,
        &mut selection,
        pt(120.0, 20.0),
        Modifiers::new(true, false),
        &mut AnchorIdMinter::new(1),
    );
    assert_eq!(selection.ids().len(), 2);
    assert!(selection.contains(second));
}

#[test]
fn ac28_a_shift_press_inside_the_sole_selected_box_away_from_every_object_finds_nothing() {
    let document = Document::new(1);
    let hollow = square(&document, 0.0, 0.0, 40.0);
    let selection = selected(&[hollow]);
    assert_eq!(
        press(&document, &selection, pt(10.0, 10.0), true),
        PressTarget::Empty
    );
    // Without Shift the same press moves the selected object.
    assert_eq!(
        press(&document, &selection, pt(10.0, 10.0), false),
        PressTarget::InsideSelectedBox
    );
}

/// Criterion 29, the example: a 100 mm filled rectangle R is selected and a
/// 10 mm filled ellipse E lies on top of it.
#[test]
fn ac29_a_press_inside_the_selected_box_goes_to_a_filled_object_above() {
    let document = Document::new(1);
    let r = square(&document, 0.0, 0.0, 100.0);
    fill(&document, r);
    let e = circle(&document, 30.0, 30.0, 5.0);
    fill(&document, e);
    let selection = selected(&[r]);
    assert_eq!(
        press(&document, &selection, pt(30.0, 30.0), false),
        PressTarget::Object(e)
    );
    assert_eq!(
        press(&document, &selection, pt(20.0, 20.0), false),
        PressTarget::InsideSelectedBox
    );

    // The press selects E, and the drag from it moves E.
    let objs = objects(&document);
    let mut tool = SelectTool::new();
    let mut selection = selected(&[r]);
    tool.pointer_down(
        &objs,
        &mut selection,
        pt(30.0, 30.0),
        tolerance(),
        tolerances(),
        false,
    );
    assert_eq!(selection.ids(), [e]);
    assert!(tool.live_move(pt(40.0, 30.0), false, false).is_some());
}

#[test]
fn ac29_an_unfilled_object_its_outline_and_an_object_below_never_take_the_press() {
    let document = Document::new(1);
    let below = circle(&document, 30.0, 30.0, 5.0);
    fill(&document, below);
    let s = square(&document, 0.0, 0.0, 100.0);
    fill(&document, s);
    let hollow_above = circle(&document, 20.0, 20.0, 5.0);
    let selection = selected(&[s]);
    // Inside the filled circle that lies BELOW the selection: the selection moves.
    assert_eq!(
        press(&document, &selection, pt(30.0, 30.0), false),
        PressTarget::InsideSelectedBox
    );
    // Inside and on the outline of the hollow circle above: still the move.
    assert_eq!(
        press(&document, &selection, pt(20.0, 20.0), false),
        PressTarget::InsideSelectedBox
    );
    assert_eq!(
        press(&document, &selection, pt(25.0, 20.0), false),
        PressTarget::InsideSelectedBox
    );
    let _ = (below, hollow_above);
}

#[test]
fn ac29_the_topmost_of_several_filled_objects_above_wins() {
    let document = Document::new(1);
    let s = square(&document, 0.0, 0.0, 100.0);
    let lower = circle(&document, 30.0, 30.0, 10.0);
    fill(&document, lower);
    let upper = circle(&document, 30.0, 30.0, 5.0);
    fill(&document, upper);
    let selection = selected(&[s]);
    assert_eq!(
        press(&document, &selection, pt(30.0, 30.0), false),
        PressTarget::Object(upper)
    );
    assert_eq!(
        press(&document, &selection, pt(38.0, 30.0), false),
        PressTarget::Object(lower)
    );
}

#[test]
fn ac29_a_shift_press_is_not_redirected() {
    let document = Document::new(1);
    let r = square(&document, 0.0, 0.0, 100.0);
    fill(&document, r);
    let e = circle(&document, 30.0, 30.0, 5.0);
    fill(&document, e);
    let selection = selected(&[r]);
    // With Shift the order is: an outline or filled-interior hit toggles. The
    // topmost hit at the point is E, whose interior is filled.
    assert_eq!(
        press(&document, &selection, pt(30.0, 30.0), true),
        PressTarget::Object(e)
    );
    // Away from E the Shift press hits R's own filled interior and toggles R.
    assert_eq!(
        press(&document, &selection, pt(20.0, 20.0), true),
        PressTarget::Object(r)
    );
}
