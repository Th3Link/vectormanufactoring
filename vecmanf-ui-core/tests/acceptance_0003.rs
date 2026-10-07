//! Black-box acceptance tests for `specs/0003-primitive-shapes/
//! specification.md`'s 22 acceptance criteria, written against
//! `vecmanf-ui-core`'s public API (`RectangleTool`, `EllipseTool`,
//! `PolygonStarTool`, `ObjectSelection`, `NodeTool`) and `vecmanf-document-core`'s own public
//! `Document`, before reading the implementation diff. Complements
//! `vecmanf-document-core/tests/acceptance_0003.rs`, which covers the
//! parts of these criteria that don't need a pointer-drag state machine.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_document_core::{Document, InnerRatio, NodeId, PathSnapshot, Point, PointCount, Shape};
use vecmanf_ui_core::{
    CreateOutcome, EllipseTool, Modifiers, NodeTool, ObjectSelection, PolyStarMode,
    PolygonStarTool, RectangleTool,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

// ---------------------------------------------------------------------
// AC1 / AC2: rectangle create-drag
// ---------------------------------------------------------------------

#[test]
fn ac1_drag_creates_a_rect_sized_from_a_to_b_zero_radius() {
    let document = Document::new(1);
    let mut tool = RectangleTool::new();
    tool.pointer_down(pt(10.0, 10.0));
    let up = tool.pointer_up(&document, pt(30.0, 25.0), false);
    let CreateOutcome::Created(id) = up else {
        panic!("expected Created, got {up:?}");
    };

    let Shape::Rect {
        bounds,
        corner_radius,
    } = document.primitive(id).unwrap().shape
    else {
        panic!("expected rect");
    };
    assert_eq!(bounds.origin, pt(10.0, 10.0));
    assert!((bounds.width.as_mm() - 20.0).abs() < 1e-9);
    assert!((bounds.height.as_mm() - 15.0).abs() < 1e-9);
    assert!(corner_radius.as_mm().abs() < 1e-9);
}

#[test]
fn ac1_a_plain_click_with_no_movement_creates_nothing() {
    let document = Document::new(1);
    let mut tool = RectangleTool::new();
    tool.pointer_down(pt(5.0, 5.0));
    let up = tool.pointer_up(&document, pt(5.0, 5.0), false);
    assert_eq!(up, CreateOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0, "nothing must be created");
}

#[test]
fn ac2_ctrl_constrain_makes_a_square_sized_to_the_larger_extent() {
    let document = Document::new(1);
    let mut tool = RectangleTool::new();
    tool.pointer_down(pt(0.0, 0.0));
    // Drag 10mm right, 30mm down: larger extent is 30 -> square 30x30.
    let up = tool.pointer_up(&document, pt(10.0, 30.0), true);
    let CreateOutcome::Created(id) = up else {
        panic!("expected Created")
    };
    let Shape::Rect { bounds, .. } = document.primitive(id).unwrap().shape else {
        panic!("expected rect")
    };
    assert!((bounds.width.as_mm() - 30.0).abs() < 1e-9);
    assert!((bounds.height.as_mm() - 30.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------
// AC3: resizing a selected rectangle via its handle
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// AC7 / AC8: ellipse create-drag
// ---------------------------------------------------------------------

#[test]
fn ac7_drag_creates_an_ellipse_with_half_extent_radii() {
    let document = Document::new(1);
    let mut tool = EllipseTool::new();
    tool.pointer_down(pt(0.0, 0.0));
    let up = tool.pointer_up(&document, pt(20.0, 10.0), false);
    let CreateOutcome::Created(id) = up else {
        panic!("expected Created")
    };
    let Shape::Ellipse { frame } = document.primitive(id).unwrap().shape else {
        panic!("expected ellipse")
    };
    assert_eq!(frame.center, pt(10.0, 5.0));
    assert!((frame.rx.as_mm() - 10.0).abs() < 1e-9);
    assert!((frame.ry.as_mm() - 5.0).abs() < 1e-9);
}

#[test]
fn ac7_a_equals_b_creates_nothing() {
    let document = Document::new(1);
    let mut tool = EllipseTool::new();
    tool.pointer_down(pt(7.0, 7.0));
    let up = tool.pointer_up(&document, pt(7.0, 7.0), false);
    assert_eq!(up, CreateOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0);
}

#[test]
fn ac8_ctrl_constrain_makes_a_circle() {
    let document = Document::new(1);
    let mut tool = EllipseTool::new();
    tool.pointer_down(pt(0.0, 0.0));
    let up = tool.pointer_up(&document, pt(4.0, 20.0), true);
    let CreateOutcome::Created(id) = up else {
        panic!("expected Created")
    };
    let Shape::Ellipse { frame } = document.primitive(id).unwrap().shape else {
        panic!("expected ellipse")
    };
    assert!((frame.rx.as_mm() - frame.ry.as_mm()).abs() < 1e-9);
    assert!((frame.rx.as_mm() - 10.0).abs() < 1e-9); // half of the larger (20) extent
}

// ---------------------------------------------------------------------
// AC10 (tool-side persistence): point count/ratio survive across shapes
// ---------------------------------------------------------------------

#[test]
fn ac10_point_count_control_persists_across_shapes_not_reset() {
    let document = Document::new(1);
    let mut tool = PolygonStarTool::new();
    tool.set_point_count(PointCount::new(9).unwrap());

    tool.pointer_down(pt(0.0, 0.0));
    let up1 = tool.pointer_up(&document, pt(10.0, 0.0), Modifiers::NONE);
    let CreateOutcome::Created(id1) = up1 else {
        panic!("expected Created")
    };

    tool.pointer_down(pt(100.0, 0.0));
    let up2 = tool.pointer_up(&document, pt(110.0, 0.0), Modifiers::NONE);
    let CreateOutcome::Created(id2) = up2 else {
        panic!("expected Created")
    };

    for id in [id1, id2] {
        let Shape::Polygon { point_count, .. } = document.primitive(id).unwrap().shape else {
            panic!("expected polygon")
        };
        assert_eq!(
            point_count.get(),
            9,
            "point count must persist, not reset between shapes"
        );
    }
}

// ---------------------------------------------------------------------
// AC11 / AC12: polygon/star create-drag, zero-movement creates nothing
// ---------------------------------------------------------------------

#[test]
fn ac11_polygon_drag_centers_at_a_one_vertex_at_b() {
    let document = Document::new(1);
    let mut tool = PolygonStarTool::new();
    tool.set_mode(PolyStarMode::Polygon);
    tool.pointer_down(pt(0.0, 0.0));
    let up = tool.pointer_up(&document, pt(10.0, 0.0), Modifiers::NONE);
    let CreateOutcome::Created(id) = up else {
        panic!("expected Created")
    };
    let Shape::Polygon { frame, .. } = document.primitive(id).unwrap().shape else {
        panic!("expected polygon")
    };
    assert_eq!(frame.center, pt(0.0, 0.0));
    assert!((frame.radius.as_mm() - 10.0).abs() < 1e-9);
}

#[test]
fn ac11_zero_movement_polygon_drag_creates_nothing() {
    let document = Document::new(1);
    let mut tool = PolygonStarTool::new();
    tool.pointer_down(pt(3.0, 3.0));
    let up = tool.pointer_up(&document, pt(3.0, 3.0), Modifiers::NONE);
    assert_eq!(up, CreateOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0);
}

#[test]
fn ac12_star_drag_creates_outer_and_inner_vertices_at_the_set_ratio() {
    let document = Document::new(1);
    let mut tool = PolygonStarTool::new();
    tool.set_mode(PolyStarMode::Star);
    tool.set_ratio(InnerRatio::new(0.3).unwrap());
    tool.pointer_down(pt(0.0, 0.0));
    let up = tool.pointer_up(&document, pt(10.0, 0.0), Modifiers::NONE);
    let CreateOutcome::Created(id) = up else {
        panic!("expected Created")
    };
    let Shape::Star {
        frame, inner_ratio, ..
    } = document.primitive(id).unwrap().shape
    else {
        panic!("expected star")
    };
    assert!((frame.radius.as_mm() - 10.0).abs() < 1e-9);
    assert!((inner_ratio.get() - 0.3).abs() < 1e-9);
}

#[test]
fn ac12_zero_movement_star_drag_creates_nothing() {
    let document = Document::new(1);
    let mut tool = PolygonStarTool::new();
    tool.set_mode(PolyStarMode::Star);
    tool.pointer_down(pt(9.0, 9.0));
    let up = tool.pointer_up(&document, pt(9.0, 9.0), Modifiers::NONE);
    assert_eq!(up, CreateOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0);
}

// ---------------------------------------------------------------------
// AC13 / AC14 / AC15: polygon/star handle drags and live controls
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// AC21: no implicit conversion
// ---------------------------------------------------------------------

#[test]
fn ac21_selecting_and_deselecting_a_primitive_never_converts_it() {
    let document = Document::new(1);
    let id = document.create_rect(vecmanf_document_core::RectBounds::from_corners(
        pt(0.0, 0.0),
        pt(10.0, 10.0),
    ));
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    selection.clear();
    selection.select_single(id);
    assert!(
        document.primitive(id).is_some(),
        "still a primitive after selection churn"
    );
}

// ---------------------------------------------------------------------
// AC22: "the resulting path objects remain selected together afterward"
// ---------------------------------------------------------------------

/// AC22's exact wording: "two or more primitives selected together...
/// each selected primitive is converted independently per criteria
/// 17-20, and the resulting path objects remain selected together
/// afterward." This replicates exactly what `vecmanf-editor-wasm`'s
/// `Session::convert_selected_to_paths` does with this crate's public
/// API (mint anchors via `outline_of`, call `Document::convert_to_paths`
/// once for the whole selection, then hand each converted path to
/// `NodeTool::select_all_anchors`), to check whether "selected together"
/// actually holds afterward.
///
/// It does not: [`NodeSelection`]/[`NodeTool`] carries a single
/// `path: Option<NodeId>` (see `vecmanf-ui-core/src/selection.rs`), so a
/// second `select_all_anchors` call for the second converted path wipes
/// out the first path's selection entirely. After a 2-object conversion,
/// at most one of the two resulting paths has anything selected — not
/// "together", not even "both, separately". This is a real AC22 gap,
/// not just a cosmetic approximation.
#[test]
fn ac22_multi_object_conversion_cannot_keep_both_paths_selected_together() {
    let document = Document::new(1);
    let rect_id = document.create_rect(vecmanf_document_core::RectBounds::from_corners(
        pt(0.0, 0.0),
        pt(20.0, 10.0),
    ));
    let ellipse_id = document.create_ellipse(vecmanf_document_core::EllipseFrame::from_corners(
        pt(0.0, 0.0),
        pt(10.0, 10.0),
    ));

    let mut selection = ObjectSelection::new();
    selection.select_single(rect_id);
    selection.toggle(ellipse_id);
    assert_eq!(
        selection.ids().len(),
        2,
        "both primitives selected together before conversion"
    );

    let mut next_anchor_counter: u64 = 0;
    let mut mint = |id: NodeId| -> Vec<vecmanf_document_core::NewAnchor> {
        vecmanf_document_core::outline_of(&document.primitive(id).unwrap().shape)
            .into_iter()
            .map(|a| {
                next_anchor_counter += 1;
                vecmanf_document_core::NewAnchor {
                    id: vecmanf_document_core::AnchorId::new(1, next_anchor_counter),
                    point: a.point,
                    handle_in: a.handle_in,
                    handle_out: a.handle_out,
                    kind: a.kind,
                }
            })
            .collect()
    };
    let conversions = vec![(rect_id, mint(rect_id)), (ellipse_id, mint(ellipse_id))];
    document.convert_to_paths(&conversions).unwrap();

    // Both really did become paths.
    let rect_path = as_path(&document, rect_id);
    let ellipse_path = as_path(&document, ellipse_id);

    // Reproduce `Session::convert_selected_to_paths`'s own reselection:
    // each converted path in turn gets its anchors selected.
    let mut node = NodeTool::new();
    node.select_all_anchors(&rect_path);
    node.select_all_anchors(&ellipse_path);

    // AC22 asks for both paths "selected together". What actually
    // happens: only the *last* one retains any selection at all.
    assert_eq!(node.selection().path(), Some(ellipse_id));
    assert!(
        !node.selection().nodes().is_empty(),
        "the last-selected path does carry anchors"
    );

    // Flip the order and confirm the *other* path loses its selection
    // instead — proving this is a structural "last write wins", not an
    // incidental test-order artifact.
    let mut node2 = NodeTool::new();
    node2.select_all_anchors(&ellipse_path);
    node2.select_all_anchors(&rect_path);
    assert_eq!(node2.selection().path(), Some(rect_id));

    // There is no representation, in this crate's selection model, of
    // "both of these paths are selected together" at the same time.
}

fn as_path(document: &Document, id: NodeId) -> PathSnapshot {
    match document.object(id).expect("object exists") {
        vecmanf_document_core::ObjectSnapshot::Path(path) => path,
        vecmanf_document_core::ObjectSnapshot::Primitive(_) => {
            panic!("expected {id:?} to have converted to a path")
        }
    }
}

// ---------------------------------------------------------------------
// Escape: discards an in-progress drag, writes nothing
// ---------------------------------------------------------------------

#[test]
fn escape_during_a_create_drag_writes_nothing() {
    let document = Document::new(1);
    let mut tool = RectangleTool::new();
    tool.pointer_down(pt(0.0, 0.0));
    assert!(tool.escape());
    let up = tool.pointer_up(&document, pt(50.0, 50.0), false);
    assert_eq!(up, CreateOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0);
}
