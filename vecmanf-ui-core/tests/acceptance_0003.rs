//! Black-box acceptance tests for `specs/0003-primitive-shapes/
//! specification.md`'s 22 acceptance criteria, written against
//! `vecmanf-ui-core`'s public API (`RectangleTool`, `EllipseTool`,
//! `PolygonStarTool`, `ObjectSelection`, `NodeTool`, `handles_for`,
//! `hit_test_handle`) and `vecmanf-document-core`'s own public
//! `Document`, before reading the implementation diff. Complements
//! `vecmanf-document-core/tests/acceptance_0003.rs`, which covers the
//! parts of these criteria that don't need a pointer-drag state machine.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_document_core::{
    Document, InnerRatio, Length, NodeId, PathSnapshot, Point, PointCount, Shape, Tolerance,
};
use vecmanf_ui_core::{
    EllipsePointerDownOutcome, EllipsePointerUpOutcome, EllipseTool, HandleKind, NodeTool,
    ObjectSelection, PolyStarMode, PolyStarPointerUpOutcome, PolygonStarTool,
    RectPointerDownOutcome, RectPointerUpOutcome, RectangleTool, ShapeHitTolerances, handles_for,
    hit_test_handle,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

const TOL: ShapeHitTolerances = ShapeHitTolerances {
    outline: Tolerance::from_mm(1.0),
    handle: Tolerance::from_mm(2.0),
};

// ---------------------------------------------------------------------
// AC1 / AC2: rectangle create-drag
// ---------------------------------------------------------------------

#[test]
fn ac1_drag_creates_a_rect_sized_from_a_to_b_zero_radius_and_selected() {
    let document = Document::new(1);
    let mut tool = RectangleTool::new();
    let mut selection = ObjectSelection::new();

    let down =
        RectangleTool::pointer_down(&mut tool, &[], &mut selection, pt(10.0, 10.0), TOL, false);
    assert_eq!(down, RectPointerDownOutcome::Creating);

    let up = tool.pointer_up(&document, pt(30.0, 25.0), false);
    let RectPointerUpOutcome::Created(id) = up else {
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
    let mut selection = ObjectSelection::new();
    tool.pointer_down(&[], &mut selection, pt(5.0, 5.0), TOL, false);
    let up = tool.pointer_up(&document, pt(5.0, 5.0), false);
    assert_eq!(up, RectPointerUpOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0, "nothing must be created");
}

#[test]
fn ac2_ctrl_constrain_makes_a_square_sized_to_the_larger_extent() {
    let document = Document::new(1);
    let mut tool = RectangleTool::new();
    let mut selection = ObjectSelection::new();
    tool.pointer_down(&[], &mut selection, pt(0.0, 0.0), TOL, false);
    // Drag 10mm right, 30mm down: larger extent is 30 -> square 30x30.
    let up = tool.pointer_up(&document, pt(10.0, 30.0), true);
    let RectPointerUpOutcome::Created(id) = up else {
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

#[test]
fn ac3_dragging_a_resize_handle_changes_bounds_keeps_rect_a_primitive() {
    let document = Document::new(1);
    let id = document.create_rect(vecmanf_document_core::RectBounds::from_corners(
        pt(0.0, 0.0),
        pt(40.0, 40.0),
    ));
    document
        .set_corner_radius(&[id], Length::from_mm(5.0))
        .unwrap();

    let mut tool = RectangleTool::new();
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    let snapshot = document.primitive(id).unwrap();

    // Find the SE resize handle.
    let handles = handles_for(&snapshot);
    let se_index = handles
        .iter()
        .position(|h| h.kind == HandleKind::Resize(vecmanf_ui_core::ResizeDirection::Se))
        .expect("an SE resize handle must exist");
    let se_position = handles[se_index].position;

    let down = tool.pointer_down(&[snapshot], &mut selection, se_position, TOL, false);
    assert_eq!(down, RectPointerDownOutcome::Handle);

    let up = tool.pointer_up(
        &document,
        se_position.translated(vecmanf_document_core::Vec2::new(20.0, 0.0)),
        false,
    );
    assert_eq!(up, RectPointerUpOutcome::Resized);

    let Shape::Rect {
        bounds,
        corner_radius,
    } = document.primitive(id).unwrap().shape
    else {
        panic!("still must be a rect primitive")
    };
    assert!((bounds.width.as_mm() - 60.0).abs() < 1e-6);
    // AC3: existing radius keeps its absolute length across a resize
    // that does not force it down.
    assert!((corner_radius.as_mm() - 5.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------
// AC7 / AC8: ellipse create-drag
// ---------------------------------------------------------------------

#[test]
fn ac7_drag_creates_an_ellipse_with_half_extent_radii_and_selected() {
    let document = Document::new(1);
    let mut tool = EllipseTool::new();
    let mut selection = ObjectSelection::new();
    let down = tool.pointer_down(&[], &mut selection, pt(0.0, 0.0), TOL, false);
    assert_eq!(down, EllipsePointerDownOutcome::Creating);
    let up = tool.pointer_up(&document, pt(20.0, 10.0), false);
    let EllipsePointerUpOutcome::Created(id) = up else {
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
    let mut selection = ObjectSelection::new();
    tool.pointer_down(&[], &mut selection, pt(7.0, 7.0), TOL, false);
    let up = tool.pointer_up(&document, pt(7.0, 7.0), false);
    assert_eq!(up, EllipsePointerUpOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0);
}

#[test]
fn ac8_ctrl_constrain_makes_a_circle() {
    let document = Document::new(1);
    let mut tool = EllipseTool::new();
    let mut selection = ObjectSelection::new();
    tool.pointer_down(&[], &mut selection, pt(0.0, 0.0), TOL, false);
    let up = tool.pointer_up(&document, pt(4.0, 20.0), true);
    let EllipsePointerUpOutcome::Created(id) = up else {
        panic!("expected Created")
    };
    let Shape::Ellipse { frame } = document.primitive(id).unwrap().shape else {
        panic!("expected ellipse")
    };
    assert!((frame.rx.as_mm() - frame.ry.as_mm()).abs() < 1e-9);
    assert!((frame.rx.as_mm() - 10.0).abs() < 1e-9); // half of the larger (20) extent
}

#[test]
fn ac9_resizing_an_ellipse_can_break_rx_eq_ry() {
    let document = Document::new(1);
    let id = document.create_ellipse(vecmanf_document_core::EllipseFrame {
        center: pt(0.0, 0.0),
        rx: Length::from_mm(10.0),
        ry: Length::from_mm(10.0),
    });
    let mut tool = EllipseTool::new();
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    let snapshot = document.primitive(id).unwrap();
    let handles = vecmanf_ui_core::handles_for(&snapshot);
    let e_index = handles
        .iter()
        .position(|h| h.kind == HandleKind::Resize(vecmanf_ui_core::ResizeDirection::E))
        .unwrap();
    let e_pos = handles[e_index].position;
    tool.pointer_down(&[snapshot], &mut selection, e_pos, TOL, false);
    let up = tool.pointer_up(
        &document,
        e_pos.translated(vecmanf_document_core::Vec2::new(15.0, 0.0)),
        false,
    );
    assert_eq!(up, EllipsePointerUpOutcome::Resized);
    let Shape::Ellipse { frame } = document.primitive(id).unwrap().shape else {
        panic!("expected ellipse")
    };
    // Dragging the E handle moves only the east edge (bounding-box
    // resize, opposite/west edge fixed at x = -10): new bbox is
    // [-10, 25], so rx = 17.5 and the center shifts to x = 7.5.
    assert!((frame.rx.as_mm() - 17.5).abs() < 1e-6);
    assert!((frame.ry.as_mm() - 10.0).abs() < 1e-9);
    assert!((frame.center.x - 7.5).abs() < 1e-6);
    assert!(
        (frame.rx.as_mm() - frame.ry.as_mm()).abs() > 1e-6,
        "AC9: resizing must be able to break rx == ry"
    );
}

// ---------------------------------------------------------------------
// AC10 (tool-side persistence): point count/ratio survive across shapes
// ---------------------------------------------------------------------

#[test]
fn ac10_point_count_control_persists_across_shapes_not_reset() {
    let document = Document::new(1);
    let mut tool = PolygonStarTool::new();
    let mut selection = ObjectSelection::new();
    tool.set_point_count(PointCount::new(9).unwrap(), &document, &selection);

    tool.pointer_down(&[], &mut selection, pt(0.0, 0.0), TOL, false);
    let up1 = tool.pointer_up(&document, pt(10.0, 0.0));
    let PolyStarPointerUpOutcome::Created(id1) = up1 else {
        panic!("expected Created")
    };

    tool.pointer_down(&[], &mut selection, pt(100.0, 0.0), TOL, false);
    let up2 = tool.pointer_up(&document, pt(110.0, 0.0));
    let PolyStarPointerUpOutcome::Created(id2) = up2 else {
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
    let mut selection = ObjectSelection::new();
    tool.pointer_down(&[], &mut selection, pt(0.0, 0.0), TOL, false);
    let up = tool.pointer_up(&document, pt(10.0, 0.0));
    let PolyStarPointerUpOutcome::Created(id) = up else {
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
    let mut selection = ObjectSelection::new();
    tool.pointer_down(&[], &mut selection, pt(3.0, 3.0), TOL, false);
    let up = tool.pointer_up(&document, pt(3.0, 3.0));
    assert_eq!(up, PolyStarPointerUpOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0);
}

#[test]
fn ac12_star_drag_creates_outer_and_inner_vertices_at_the_set_ratio() {
    let document = Document::new(1);
    let mut tool = PolygonStarTool::new();
    tool.set_mode(PolyStarMode::Star);
    let mut selection = ObjectSelection::new();
    tool.set_ratio(InnerRatio::new(0.3).unwrap(), &document, &selection);
    tool.pointer_down(&[], &mut selection, pt(0.0, 0.0), TOL, false);
    let up = tool.pointer_up(&document, pt(10.0, 0.0));
    let PolyStarPointerUpOutcome::Created(id) = up else {
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
    let mut selection = ObjectSelection::new();
    tool.pointer_down(&[], &mut selection, pt(9.0, 9.0), TOL, false);
    let up = tool.pointer_up(&document, pt(9.0, 9.0));
    assert_eq!(up, PolyStarPointerUpOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0);
}

// ---------------------------------------------------------------------
// AC13 / AC14 / AC15: polygon/star handle drags and live controls
// ---------------------------------------------------------------------

#[test]
fn ac13_resize_handle_scales_uniformly_keeping_count_and_ratio() {
    let document = Document::new(1);
    let id = document.create_star(
        vecmanf_document_core::StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0)),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let mut tool = PolygonStarTool::new();
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    let snapshot = document.primitive(id).unwrap();
    let handles = handles_for(&snapshot);
    let resize = handles
        .iter()
        .find(|h| matches!(h.kind, HandleKind::Resize(_)))
        .unwrap();
    let pos = resize.position;
    tool.pointer_down(&[snapshot], &mut selection, pos, TOL, false);
    // Drag outward from center along the same direction to double the radius.
    let direction = pos.vector_to(pt(0.0, 0.0)).negated().normalized_to(1.0);
    let new_pos = pos.translated(direction.scaled(10.0));
    let up = tool.pointer_up(&document, new_pos);
    assert_eq!(up, PolyStarPointerUpOutcome::Resized);
    let Shape::Star {
        frame,
        point_count,
        inner_ratio,
    } = document.primitive(id).unwrap().shape
    else {
        panic!("expected star")
    };
    assert!((frame.radius.as_mm() - 20.0).abs() < 1e-6);
    assert_eq!(point_count.get(), 5);
    assert!((inner_ratio.get() - 0.5).abs() < 1e-9);
}

#[test]
fn ac14_inner_radius_handle_changes_ratio_keeps_outer_radius() {
    let document = Document::new(1);
    let id = document.create_star(
        vecmanf_document_core::StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0)),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let mut tool = PolygonStarTool::new();
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    let snapshot = document.primitive(id).unwrap();
    let handles = handles_for(&snapshot);
    let inner = handles
        .iter()
        .find(|h| h.kind == HandleKind::InnerRadius)
        .expect("a star must show an inner-radius handle");
    let pos = inner.position;
    tool.pointer_down(&[snapshot], &mut selection, pos, TOL, false);
    let direction = pt(0.0, 0.0).vector_to(pos).normalized_to(1.0);
    let new_pos = pos.translated(direction.scaled(1.0)); // push inner vertex farther out
    let up = tool.pointer_up(&document, new_pos);
    assert_eq!(up, PolyStarPointerUpOutcome::InnerRatioChanged);
    let Shape::Star {
        frame, inner_ratio, ..
    } = document.primitive(id).unwrap().shape
    else {
        panic!("expected star")
    };
    assert!(
        (frame.radius.as_mm() - 10.0).abs() < 1e-9,
        "outer radius must stay fixed"
    );
    assert!(inner_ratio.get() > 0.5, "ratio must have increased");
}

#[test]
fn ac14_a_plain_polygon_shows_no_inner_radius_handle() {
    let document = Document::new(1);
    let id = document.create_polygon(
        vecmanf_document_core::StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0)),
        PointCount::new(5).unwrap(),
    );
    let snapshot = document.primitive(id).unwrap();
    let handles = handles_for(&snapshot);
    assert!(
        !handles.iter().any(|h| h.kind == HandleKind::InnerRadius),
        "a polygon has no inner radius distinct from its outer one"
    );
}

#[test]
fn ac15_point_count_change_updates_the_selected_shape_live_keeping_size_and_ratio() {
    let document = Document::new(1);
    let id = document.create_star(
        vecmanf_document_core::StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0)),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let mut tool = PolygonStarTool::new();
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    tool.set_point_count(PointCount::new(12).unwrap(), &document, &selection);
    let Shape::Star {
        frame,
        point_count,
        inner_ratio,
    } = document.primitive(id).unwrap().shape
    else {
        panic!("expected star")
    };
    assert_eq!(point_count.get(), 12);
    assert!((frame.radius.as_mm() - 10.0).abs() < 1e-9);
    assert!((inner_ratio.get() - 0.4).abs() < 1e-9);
}

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
    let mut selection = ObjectSelection::new();
    tool.pointer_down(&[], &mut selection, pt(0.0, 0.0), TOL, false);
    assert!(tool.escape());
    let up = tool.pointer_up(&document, pt(50.0, 50.0), false);
    assert_eq!(up, RectPointerUpOutcome::NoOp);
    assert_eq!(document.object_ids().len(), 0);
}

#[test]
fn hit_test_handle_ignores_non_draggable_echo_handles() {
    let document = Document::new(1);
    let id = document.create_rect(vecmanf_document_core::RectBounds::from_corners(
        pt(0.0, 0.0),
        pt(40.0, 40.0),
    ));
    document
        .set_corner_radius(&[id], Length::from_mm(8.0))
        .unwrap();
    let snapshot = document.primitive(id).unwrap();
    let handles = handles_for(&snapshot);
    let echo = handles
        .iter()
        .position(|h| h.kind == HandleKind::CornerRadiusEcho)
        .expect("rounded rect must show echo handles at the other 3 corners");
    assert!(!handles[echo].draggable);
    let hit = hit_test_handle(&handles, handles[echo].position, TOL.handle);
    assert_ne!(
        hit,
        Some(echo),
        "a non-draggable echo handle must never itself be the hit"
    );
}
