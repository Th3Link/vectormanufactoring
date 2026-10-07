//! Tests of `curvyo-ui-core`'s share of PR 1 of
//! `specs/unified-object-editing/specification.md`: the parameter handles
//! next to the transform handles (criteria 1 to 9), the press order (35),
//! the stored fields (24). Session-level behaviour (commit counts, cursors,
//! readouts, the bar) is in `curvyo-editor-wasm/tests/unified_object_editing.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use std::fmt::Write;

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot,
    Point, PointCount, PrimitiveSnapshot, RectBounds, Shape, StarFrame, Tolerance, Vec2,
};
use curvyo_ui_core::{
    AnchorIdMinter, Corner, EditHandle, Modifiers, ObjectSelection, ParamHandle, ResizeDirection,
    SelectPointerDownOutcome, SelectTool, TransformHandleTolerances, radius_gain, radius_travel,
};
use proptest::prelude::*;

const SEGMENT_TOLERANCE: Tolerance = Tolerance::from_mm(1.0);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Two screen pixels per millimetre.
fn tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(2.0)
}

struct Rig {
    document: Document,
    id: NodeId,
    selection: ObjectSelection,
    tool: SelectTool,
}

impl Rig {
    fn rect(width: f64, height: f64, radius: f64) -> Self {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: pt(10.0, 20.0),
            width: Length::from_mm(width),
            height: Length::from_mm(height),
        });
        document
            .set_corner_radius(&[id], Length::from_mm(radius))
            .unwrap();
        Self::selected(document, id)
    }

    fn star(radius: f64, points: u32, ratio: f64) -> Self {
        let document = Document::new(1);
        let id = document.create_star(
            StarFrame {
                center: pt(60.0, 50.0),
                radius: Length::from_mm(radius),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(points).unwrap(),
            InnerRatio::new(ratio).unwrap(),
        );
        Self::selected(document, id)
    }

    fn selected(document: Document, id: NodeId) -> Self {
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        Self {
            document,
            id,
            selection,
            tool: SelectTool::new(),
        }
    }

    fn object(&self) -> ObjectSnapshot {
        self.document.object(self.id).unwrap()
    }

    fn objects(&self) -> Vec<ObjectSnapshot> {
        vec![self.object()]
    }

    fn handles(&self) -> Vec<(EditHandle, Point)> {
        SelectTool::transform_handles(&self.objects(), &self.selection, tolerances(), false)
    }

    fn handle(&self, wanted: EditHandle) -> Point {
        self.handles()
            .into_iter()
            .find(|(h, _)| *h == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} is not drawn"))
            .1
    }

    fn press(&mut self, at: Point) -> SelectPointerDownOutcome {
        let objects = self.objects();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            at,
            SEGMENT_TOLERANCE,
            tolerances(),
            false,
        )
    }

    fn hit(&self, at: Point) -> Option<EditHandle> {
        SelectTool::handle_at(&self.objects(), &self.selection, at, tolerances(), false)
            .map(|(_, _, h)| h)
    }

    fn release(&mut self, at: Point) {
        let objects = self.objects();
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            at,
            Modifiers::new(false, false),
            &mut AnchorIdMinter::new(99),
        );
    }

    fn radius(&self) -> f64 {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radius, .. },
            ..
        }) = self.object()
        else {
            panic!("a rectangle");
        };
        corner_radius.as_mm()
    }

    fn ratio(&self) -> f64 {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Star { inner_ratio, .. },
            ..
        }) = self.object()
        else {
            panic!("a star");
        };
        inner_ratio.get()
    }
}

fn count_params(handles: &[(EditHandle, Point)]) -> usize {
    handles
        .iter()
        .filter(|(h, _)| matches!(h, EditHandle::Param(_)))
        .count()
}

/// Criterion 1: a selected rectangle shows its transform handles and four
/// corner-radius handles at once, at radius 0 too; a star one inner-radius
/// handle; a polygon none.
#[test]
fn ac1_a_rectangle_shows_four_radius_handles_with_its_transform_handles() {
    let rig = Rig::rect(100.0, 60.0, 0.0);
    let handles = rig.handles();
    assert_eq!(count_params(&handles), 4);
    for corner in Corner::ALL {
        assert!(
            handles
                .iter()
                .any(|(h, _)| *h == EditHandle::Param(ParamHandle::CornerRadius(corner)))
        );
    }
    assert!(
        handles
            .iter()
            .any(|(h, _)| matches!(h, EditHandle::Resize(ResizeDirection::N))),
        "the edge resize handles are there too"
    );
    assert!(
        handles
            .iter()
            .any(|(h, _)| matches!(h, EditHandle::Rotate(_)))
    );
    assert!(
        handles.iter().any(|(h, _)| *h == EditHandle::Move),
        "the centre handle is drawn at radius 0"
    );
}

#[test]
fn ac1_a_star_shows_one_inner_radius_handle_and_a_polygon_none() {
    let star = Rig::star(40.0, 5, 0.5);
    let handles = star.handles();
    assert_eq!(count_params(&handles), 1);
    assert!(
        handles
            .iter()
            .any(|(h, _)| *h == EditHandle::Param(ParamHandle::InnerRadius))
    );
    let document = Document::new(1);
    let id = document.create_polygon(
        StarFrame {
            center: pt(60.0, 50.0),
            radius: Length::from_mm(40.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(6).unwrap(),
    );
    assert_eq!(count_params(&Rig::selected(document, id).handles()), 0);
}

/// Criterion 7: the tiers 24, 48, 72 on the shorter side, and the order in
/// which handles disappear.
#[test]
fn ac7_the_tiers_follow_the_shorter_side_in_screen_pixels() {
    let handle_counts = |side_mm: f64| {
        let rig = Rig::rect(side_mm * 3.0, side_mm, 0.0);
        let box_ = curvyo_ui_core::oriented_bounds(&rig.object());
        let handles = rig.handles();
        let resize_edges = handles
            .iter()
            .filter(|(h, _)| {
                matches!(h, EditHandle::Resize(d) if !curvyo_ui_core::is_corner(*d))
                    && curvyo_ui_core::is_drawn_handle(*h, &box_, &tolerances())
            })
            .count();
        let centre = handles.iter().any(|(h, _)| *h == EditHandle::Move);
        (resize_edges, centre, count_params(&handles))
    };
    // Two pixels per millimetre.
    assert_eq!(handle_counts(11.9), (0, false, 0), "23.8 px");
    assert_eq!(handle_counts(12.0), (4, false, 0), "24 px");
    assert_eq!(handle_counts(23.9), (4, false, 0), "47.8 px");
    assert_eq!(handle_counts(24.0), (4, true, 0), "48 px");
    assert_eq!(handle_counts(35.9), (4, true, 0), "71.8 px");
    assert_eq!(handle_counts(36.0), (4, true, 4), "72 px");
}

/// Criterion 2: the radius handle at radius 0 is 15 px from its corner along
/// the inward diagonal; the handle follows the pointer exactly; all four
/// move in step.
#[test]
fn ac2_a_radius_drag_keeps_the_handle_under_the_pointer_and_moves_all_four() {
    let mut rig = Rig::rect(100.0, 60.0, 0.0);
    let corner = EditHandle::Param(ParamHandle::CornerRadius(Corner::Tr));
    let start = rig.handle(corner);
    let diagonal = Vec2::new(-1.0, 1.0).normalized_to(1.0);
    assert!(
        (pt(110.0, 20.0).vector_to(start).length() - 7.5).abs() < 1e-9,
        "15 px at 2 px/mm"
    );
    assert_eq!(rig.press(start), SelectPointerDownOutcome::Handle);
    let travel = 6.0;
    let to = start.translated(diagonal.scaled(travel));
    rig.release(to);
    assert!(rig.radius() > 0.0);
    let gain = radius_gain(60.0, &tolerances());
    assert!((rig.radius() - gain * travel).abs() < 1e-9);
    // The handle sits under the pointer after the commit.
    let after = rig.handle(corner);
    assert!(after.vector_to(to).length() < 1e-9, "{after:?} vs {to:?}");
    // The other three followed: every handle is `inset + rho * L` from its
    // own corner, for the one radius.
    let rho = rig.radius() / 30.0;
    let expected = tolerances().param_inset_mm + rho * radius_travel(60.0, &tolerances());
    let corners = [
        (Corner::Tl, pt(10.0, 20.0)),
        (Corner::Tr, pt(110.0, 20.0)),
        (Corner::Br, pt(110.0, 80.0)),
        (Corner::Bl, pt(10.0, 80.0)),
    ];
    for (other, at) in corners {
        let here = rig.handle(EditHandle::Param(ParamHandle::CornerRadius(other)));
        assert!(
            (at.vector_to(here).length() - expected).abs() < 1e-9,
            "{other:?}"
        );
    }
}

/// Criterion 2: dragging back to or beyond the 15 px point gives exactly 0,
/// dragging far out is limited to half the shorter side.
#[test]
fn ac2_the_radius_is_zero_at_the_inset_point_and_clamped_at_half_the_side() {
    let mut rig = Rig::rect(100.0, 60.0, 10.0);
    let corner = EditHandle::Param(ParamHandle::CornerRadius(Corner::Tl));
    let start = rig.handle(corner);
    rig.press(start);
    rig.release(start.translated(Vec2::new(-30.0, -30.0)));
    assert_eq!(rig.radius(), 0.0);

    let mut rig = Rig::rect(100.0, 60.0, 0.0);
    let start = rig.handle(corner);
    rig.press(start);
    rig.release(start.translated(Vec2::new(500.0, 500.0)));
    assert!((rig.radius() - 30.0).abs() < 1e-9, "half of 60");
}

/// Criterion 2: the zero position is the 15 px point: a pointer exactly
/// there is radius 0.
#[test]
fn ac2_a_press_and_a_release_beyond_the_inset_point_is_exactly_zero() {
    let mut rig = Rig::rect(100.0, 60.0, 0.0);
    let corner = EditHandle::Param(ParamHandle::CornerRadius(Corner::Br));
    let start = rig.handle(corner);
    rig.press(start);
    let outward = Vec2::new(1.0, 1.0).normalized_to(1.0).scaled(10.0);
    rig.release(start.translated(outward));
    assert_eq!(rig.radius(), 0.0);
}

/// Criterion 3: a star's inner radius follows the handle, the outer radius
/// stays, the ratio is limited to 0.01..=0.99.
#[test]
fn ac3_the_star_inner_ratio_drag_keeps_the_outer_radius_and_is_limited() {
    let mut rig = Rig::star(40.0, 5, 0.5);
    let handle = EditHandle::Param(ParamHandle::InnerRadius);
    let start = rig.handle(handle);
    let theta = std::f64::consts::PI / 5.0;
    let outward = Vec2::new(theta.cos(), theta.sin());
    rig.press(start);
    rig.release(start.translated(outward.scaled(4.0)));
    assert!((rig.ratio() - 0.6).abs() < 1e-9);
    let ObjectSnapshot::Primitive(PrimitiveSnapshot {
        shape: Shape::Star { frame, .. },
        ..
    }) = rig.object()
    else {
        panic!("a star");
    };
    assert_eq!(frame.radius.as_mm(), 40.0);

    let mut far = Rig::star(40.0, 5, 0.5);
    let start = far.handle(handle);
    far.press(start);
    far.release(start.translated(outward.scaled(1e4)));
    assert!((far.ratio() - 0.99).abs() < 1e-12);
}

/// Criterion 4: a rotated rectangle's handles follow its rotation and a drag
/// is measured along its own axes.
#[test]
fn ac4_a_rotated_rectangle_drags_its_radius_along_its_own_axes() {
    let mut rig = Rig::rect(100.0, 60.0, 0.0);
    let turned = rig
        .object()
        .rotated(pt(60.0, 50.0), Angle::from_radians(0.7));
    rig.document.rotate_object(&turned).unwrap();
    let corner = EditHandle::Param(ParamHandle::CornerRadius(Corner::Tr));
    let start = rig.handle(corner);
    // The local inward diagonal of the NE corner, turned by 0.7 rad.
    let diagonal = Vec2::new(-1.0, 1.0)
        .normalized_to(1.0)
        .rotated(Angle::from_radians(0.7));
    rig.press(start);
    rig.release(start.translated(diagonal.scaled(5.0)));
    let gain = radius_gain(60.0, &tolerances());
    assert!((rig.radius() - gain * 5.0).abs() < 1e-9);
}

/// Criterion 5: a parameter handle has a 12 px hit radius (6 mm at 2 px/mm),
/// not the 16 px of the resize handles; and it wins an exact tie.
#[test]
fn ac5_a_parameter_handle_hits_within_twelve_pixels() {
    let rig = Rig::rect(100.0, 60.0, 0.0);
    let corner = EditHandle::Param(ParamHandle::CornerRadius(Corner::Tl));
    let at = rig.handle(corner);
    let diagonal = Vec2::new(1.0, 1.0).normalized_to(1.0);
    assert_eq!(rig.hit(at.translated(diagonal.scaled(5.9))), Some(corner));
    // 6.5 mm = 13 px from the knob: not the knob. (Toward the box centre, so
    // the corner resize handle, 7.5 + 6.5 = 14 mm = 28 px away, does not
    // claim it either.)
    assert_ne!(rig.hit(at.translated(diagonal.scaled(6.5))), Some(corner));
}

/// Criterion 5: where a parameter handle and a resize handle are exactly
/// equidistant, the parameter handle wins.
#[test]
fn ac5_a_parameter_handle_wins_an_exact_tie() {
    use curvyo_ui_core::{OrientedBox, hit_transform_handle};
    let box_ = OrientedBox {
        min: pt(0.0, 0.0),
        max: pt(100.0, 100.0),
        angle: Angle::from_radians(0.0),
        pivot: pt(50.0, 50.0),
    };
    let handles = vec![
        (EditHandle::Resize(ResizeDirection::Se), pt(10.0, 0.0)),
        (
            EditHandle::Param(ParamHandle::CornerRadius(Corner::Tl)),
            pt(-10.0, 0.0),
        ),
    ];
    let tolerances = TransformHandleTolerances {
        resize: Tolerance::from_mm(16.0),
        param_hit: Tolerance::from_mm(12.0),
        ..TransformHandleTolerances::at_scale(1.0)
    };
    // Outside the box (so the resize inner band does not apply).
    let hit = hit_transform_handle(&handles, &box_, pt(0.0, -150.0), &tolerances, false);
    assert_eq!(hit, None, "far away");
    let hit = hit_transform_handle(&handles, &box_, pt(0.0, 0.0), &tolerances, false);
    assert_eq!(
        hit,
        Some(EditHandle::Param(ParamHandle::CornerRadius(Corner::Tl)))
    );
}

/// Criterion 6: a handle that is not drawn has no hit area: a press where a
/// radius handle would be, on a box under 72 px, is a move.
#[test]
fn ac6_an_undrawn_parameter_handle_has_no_hit_area() {
    // 30 mm at 2 px/mm: 60 px, below the 72 px threshold.
    let mut rig = Rig::rect(90.0, 30.0, 0.0);
    assert_eq!(count_params(&rig.handles()), 0);
    let where_it_would_be = pt(
        10.0 + 7.5 / std::f64::consts::SQRT_2,
        20.0 + 7.5 / std::f64::consts::SQRT_2,
    );
    assert_eq!(rig.hit(where_it_would_be), None);
    assert_eq!(
        rig.press(where_it_would_be),
        SelectPointerDownOutcome::Selected
    );
    rig.release(where_it_would_be.translated(Vec2::new(5.0, 3.0)));
    let ObjectSnapshot::Primitive(PrimitiveSnapshot {
        shape: Shape::Rect {
            bounds,
            corner_radius,
        },
        ..
    }) = rig.object()
    else {
        panic!("a rectangle");
    };
    assert_eq!(corner_radius.as_mm(), 0.0, "no radius written");
    assert!((bounds.origin.x - 15.0).abs() < 1e-9, "the object moved");
}

/// Criterion 9: a press and release with less than 3 px of movement writes
/// nothing; so does Escape during a drag.
#[test]
fn ac9_a_dead_zone_press_and_an_escaped_drag_write_nothing() {
    let mut rig = Rig::rect(100.0, 60.0, 0.0);
    let corner = EditHandle::Param(ParamHandle::CornerRadius(Corner::Tr));
    let at = rig.handle(corner);
    let diagonal = Vec2::new(-1.0, 1.0).normalized_to(1.0);
    rig.press(at);
    rig.release(at.translated(diagonal.scaled(1.4))); // 2.8 px
    assert_eq!(rig.radius(), 0.0);

    rig.press(at);
    rig.tool.pointer_moved(
        at.translated(diagonal.scaled(10.0)),
        Modifiers::NONE,
        &mut rig.selection,
    );
    rig.tool.escape();
    rig.release(at.translated(diagonal.scaled(10.0)));
    assert_eq!(rig.radius(), 0.0);
}

/// Criterion 24: a radius written through the Select tool is the same field
/// the shape tools wrote, and the frame and rotation are untouched.
#[test]
fn ac24_a_radius_drag_writes_only_the_corner_radius() {
    let mut rig = Rig::rect(100.0, 60.0, 0.0);
    let before = rig.object();
    let at = rig.handle(EditHandle::Param(ParamHandle::CornerRadius(Corner::Tr)));
    rig.press(at);
    rig.release(at.translated(Vec2::new(-5.0, 5.0)));
    let (ObjectSnapshot::Primitive(a), ObjectSnapshot::Primitive(b)) = (before, rig.object())
    else {
        panic!("primitives");
    };
    let (Shape::Rect { bounds: ba, .. }, Shape::Rect { bounds: bb, .. }) = (a.shape, b.shape)
    else {
        panic!("rectangles");
    };
    assert_eq!(ba, bb);
    assert_eq!(a.rotation, b.rotation);
    assert_eq!(a.stroke_width, b.stroke_width);
    assert!(rig.radius() > 0.0);
}

/// Criterion 35: with one primitive selected, a press inside its box on no
/// handle moves it even where another object's outline lies inside the box.
#[test]
fn ac35_inside_the_selected_primitive_box_a_press_moves_it_before_any_outline_hit() {
    let document = Document::new(1);
    let big = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(100.0),
        height: Length::from_mm(100.0),
    });
    let inner = document.create_rect(RectBounds {
        origin: pt(30.0, 40.0),
        width: Length::from_mm(20.0),
        height: Length::from_mm(20.0),
    });
    let objects: Vec<ObjectSnapshot> = [big, inner]
        .iter()
        .map(|id| document.object(*id).unwrap())
        .collect();
    let mut selection = ObjectSelection::new();
    selection.select_single(big);
    let mut tool = SelectTool::new();
    // On the inner rectangle's left edge (30, 50), inside the big box and on
    // no handle (the centre handle is at (50, 50)).
    let outcome = tool.pointer_down(
        &objects,
        &mut selection,
        pt(30.0, 50.0),
        SEGMENT_TOLERANCE,
        tolerances(),
        false,
    );
    assert_eq!(outcome, SelectPointerDownOutcome::Selected);
    assert_eq!(selection.ids(), &[big], "the selection did not change");
}

// ---------------------------------------------------------------------
// Blue new, black old: preview equals release (criteria 12, 13, 14)
// ---------------------------------------------------------------------

fn rig_of(kind: u8) -> Rig {
    let document = Document::new(1);
    let id = match kind % 5 {
        0 => {
            let id = document.create_rect(RectBounds {
                origin: pt(10.0, 20.0),
                width: Length::from_mm(100.0),
                height: Length::from_mm(60.0),
            });
            document
                .set_corner_radius(&[id], Length::from_mm(8.0))
                .unwrap();
            id
        }
        1 => document.create_ellipse(EllipseFrame {
            center: pt(60.0, 50.0),
            rx: Length::from_mm(50.0),
            ry: Length::from_mm(30.0),
        }),
        2 => document.create_polygon(
            StarFrame {
                center: pt(60.0, 50.0),
                radius: Length::from_mm(40.0),
                angle: Angle::from_radians(0.3),
            },
            PointCount::new(6).unwrap(),
        ),
        3 => document.create_star(
            StarFrame {
                center: pt(60.0, 50.0),
                radius: Length::from_mm(40.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        ),
        _ => document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), pt(10.0, 20.0)),
                NewAnchor::corner(AnchorId::new(1, 2), pt(110.0, 30.0)),
                NewAnchor::corner(AnchorId::new(1, 3), pt(60.0, 90.0)),
            ],
            true,
        ),
    };
    Rig::selected(document, id)
}

proptest! {
    /// Criteria 12 and 13: for every handle of every object kind, the blue
    /// outline during a drag is exactly the geometry a release at that
    /// moment commits (same pointer, same modifiers), and where there is no
    /// preview nothing is committed.
    #[test]
    fn the_live_edit_is_what_the_release_commits(
        kind in 0u8..5,
        handle_pick in 0usize..40,
        dx in -60.0f64..60.0,
        dy in -60.0f64..60.0,
        shift in proptest::bool::ANY,
        ctrl in proptest::bool::ANY,
    ) {
        let mut rig = rig_of(kind);
        let handles: Vec<(EditHandle, Point)> = rig
            .handles()
            .into_iter()
            .filter(|(h, _)| *h != EditHandle::Move)
            .collect();
        let (handle, at) = handles[handle_pick % handles.len()];
        let before = rig.object();
        prop_assert_eq!(rig.press(at), SelectPointerDownOutcome::Handle, "{:?}", handle);
        let to = at.translated(Vec2::new(dx, dy));
        rig.tool.pointer_moved(to, Modifiers::NONE, &mut rig.selection);
        let objects = rig.objects();
        let live = rig.tool.live_edit(&objects, &rig.selection, to, shift, ctrl);
        let objects = rig.objects();
        rig.tool.pointer_up(&rig.document, &objects, &mut rig.selection, to, Modifiers::new(shift, ctrl), &mut AnchorIdMinter::new(99));
        let after = rig.object();
        match live {
            Some(live) => {
                prop_assert_eq!(live.objects.len(), 1);
                // The committed registers hold what the preview showed.
                prop_assert!(
                    same(&live.objects[0], &after),
                    "{:?}: live {:?} vs committed {:?}", handle, live.objects[0], after
                );
            }
            None => prop_assert!(same(&before, &after), "{:?}: nothing to preview, nothing written", handle),
        }
    }
}

/// Approximate equality of two snapshots through their outlines' and
/// parameters' debug form with every number rounded to 1e-7.
fn same(a: &ObjectSnapshot, b: &ObjectSnapshot) -> bool {
    round_numbers(&format!("{a:?}")) == round_numbers(&format!("{b:?}"))
}

fn round_numbers(text: &str) -> String {
    let mut out = String::new();
    let mut number = String::new();
    let flush = |number: &mut String, out: &mut String| {
        if let Ok(value) = number.parse::<f64>() {
            let _ = write!(out, "{:.7}", value + 0.0);
        } else {
            out.push_str(number);
        }
        number.clear();
    };
    for c in text.chars() {
        if c.is_ascii_digit()
            || matches!(c, '.' | '-' | 'e' | '+') && !number.is_empty()
            || c == '-' && number.is_empty()
        {
            number.push(c);
        } else {
            flush(&mut number, &mut out);
            out.push(c);
        }
    }
    flush(&mut number, &mut out);
    out
}

/// Criterion 12: a move dragged out of the dead zone and back to its start
/// shows no blue outline and writes nothing, not even a zero-offset commit.
#[test]
fn ac12_a_move_dragged_back_to_its_start_writes_nothing() {
    for kind in 0u8..5 {
        let mut rig = rig_of(kind);
        let before = rig.document.export_loro_snapshot().unwrap();
        // A press on the object's body: the centre of its box.
        let body = {
            let b = curvyo_ui_core::oriented_bounds(&rig.object());
            b.to_document(Point::new(
                b.min.x + b.width() * 0.5 + 3.0,
                b.min.y + b.height() * 0.5 + 3.0,
            ))
        };
        rig.press(body);
        let away = body.translated(Vec2::new(30.0, 12.0));
        rig.tool
            .pointer_moved(away, Modifiers::NONE, &mut rig.selection);
        let objects = rig.objects();
        assert!(
            rig.tool
                .live_edit(&objects, &rig.selection, away, false, false)
                .is_some(),
            "kind {kind}: out of the dead zone there is a preview"
        );
        rig.tool
            .pointer_moved(body, Modifiers::NONE, &mut rig.selection);
        let objects = rig.objects();
        assert!(
            rig.tool
                .live_edit(&objects, &rig.selection, body, false, false)
                .is_none(),
            "kind {kind}: back at the start there is none"
        );
        rig.release(body);
        assert_eq!(
            rig.document.export_loro_snapshot().unwrap(),
            before,
            "kind {kind}: nothing was written"
        );
    }
}

/// Criteria 10, 11: a move previews every selected object.
#[test]
fn ac11_a_multi_object_move_previews_every_selected_object() {
    let document = Document::new(1);
    let a = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    let b = document.create_rect(RectBounds {
        origin: pt(50.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    let objects: Vec<ObjectSnapshot> = [a, b]
        .iter()
        .map(|id| document.object(*id).unwrap())
        .collect();
    let mut selection = ObjectSelection::new();
    selection.toggle(a);
    selection.toggle(b);
    let mut tool = SelectTool::new();
    tool.pointer_down(
        &objects,
        &mut selection,
        pt(0.0, 5.0),
        SEGMENT_TOLERANCE,
        tolerances(),
        false,
    );
    let to = pt(10.0, 9.0);
    tool.pointer_moved(to, Modifiers::NONE, &mut selection);
    let live = tool
        .live_edit(&objects, &selection, to, false, false)
        .unwrap();
    assert_eq!(live.objects.len(), 2);
    assert_eq!(live.objects[0], objects[0].translated(Vec2::new(10.0, 4.0)));
    assert_eq!(live.objects[1], objects[1].translated(Vec2::new(10.0, 4.0)));
}

/// Criterion 14: with the pointer holding still, a Shift change reaches the
/// live edit on the next call (the resize pivot swaps to the centre).
#[test]
fn ac14_a_modifier_change_with_no_pointer_movement_changes_the_live_edit() {
    let mut rig = rig_of(0);
    let se = rig.handle(EditHandle::Resize(ResizeDirection::Se));
    rig.press(se);
    let to = se.translated(Vec2::new(10.0, 10.0));
    rig.tool
        .pointer_moved(to, Modifiers::NONE, &mut rig.selection);
    let objects = rig.objects();
    let plain = rig
        .tool
        .live_edit(&objects, &rig.selection, to, false, false)
        .unwrap();
    let centred = rig
        .tool
        .live_edit(&objects, &rig.selection, to, true, false)
        .unwrap();
    assert_ne!(plain, centred);
}

/// Criterion 12: inside the dead zone there is no preview.
#[test]
fn ac12_inside_the_dead_zone_there_is_no_live_edit() {
    let mut rig = rig_of(0);
    let tr = rig.handle(EditHandle::Param(ParamHandle::CornerRadius(Corner::Tr)));
    rig.press(tr);
    let near = tr.translated(Vec2::new(-0.5, 0.5));
    rig.tool
        .pointer_moved(near, Modifiers::NONE, &mut rig.selection);
    let objects = rig.objects();
    assert!(
        rig.tool
            .live_edit(&objects, &rig.selection, near, false, false)
            .is_none()
    );
}

// ---------------------------------------------------------------------
// Ported from the shape tools' own editing tests (deleted with them, PR 2):
// the same numbers, driven through the Select tool.
// ---------------------------------------------------------------------

/// A rig whose shape is selected, at a zoom where a 10 mm shape is wide enough
/// for its parameter handles (8 px per millimetre).
struct Zoomed {
    document: Document,
    id: NodeId,
    selection: ObjectSelection,
    tool: SelectTool,
}

fn zoomed_tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(8.0)
}

impl Zoomed {
    fn new(document: Document, id: NodeId) -> Self {
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        Self {
            document,
            id,
            selection,
            tool: SelectTool::new(),
        }
    }

    fn objects(&self) -> Vec<ObjectSnapshot> {
        vec![self.document.object(self.id).unwrap()]
    }

    fn handle(&self, wanted: EditHandle) -> Point {
        SelectTool::transform_handles(&self.objects(), &self.selection, zoomed_tolerances(), false)
            .into_iter()
            .find(|(h, _)| *h == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} is not drawn"))
            .1
    }

    fn drag(&mut self, from: Point, delta: Vec2) -> SelectPointerDownOutcome {
        let objects = self.objects();
        let outcome = self.tool.pointer_down(
            &objects,
            &mut self.selection,
            from,
            Tolerance::from_mm(0.5),
            zoomed_tolerances(),
            false,
        );
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            from.translated(delta),
            Modifiers::new(false, false),
            &mut AnchorIdMinter::new(99),
        );
        outcome
    }

    fn shape(&self) -> Shape {
        let ObjectSnapshot::Primitive(p) = self.document.object(self.id).unwrap() else {
            panic!("a primitive");
        };
        p.shape
    }
}

/// Was `RectangleTool` AC3: a resize through the SE handle changes the bounds
/// and, with "Scale corner radius" off (the default), keeps the radius's
/// absolute length.
#[test]
fn ported_ac3_a_resize_handle_changes_the_bounds_and_keeps_the_radius() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(40.0, 40.0)));
    document
        .set_corner_radius(&[id], Length::from_mm(5.0))
        .unwrap();
    let mut rig = Zoomed::new(document, id);
    let se = rig.handle(EditHandle::Resize(ResizeDirection::Se));
    assert_eq!(
        rig.drag(se, Vec2::new(20.0, 0.0)),
        SelectPointerDownOutcome::Handle
    );
    let Shape::Rect {
        bounds,
        corner_radius,
    } = rig.shape()
    else {
        panic!("still a rectangle");
    };
    assert!((bounds.width.as_mm() - 60.0).abs() < 1e-6);
    assert!((corner_radius.as_mm() - 5.0).abs() < 1e-9);
}

/// Was `EllipseTool` AC9: the E handle moves only the east edge, so a resize
/// breaks rx == ry: bounding box [-10, 25], rx 17.5, centre x 7.5.
#[test]
fn ported_ac9_resizing_an_ellipse_can_break_rx_eq_ry() {
    let document = Document::new(1);
    let id = document.create_ellipse(EllipseFrame {
        center: pt(0.0, 0.0),
        rx: Length::from_mm(10.0),
        ry: Length::from_mm(10.0),
    });
    let mut rig = Zoomed::new(document, id);
    let e = rig.handle(EditHandle::Resize(ResizeDirection::E));
    rig.drag(e, Vec2::new(15.0, 0.0));
    let Shape::Ellipse { frame } = rig.shape() else {
        panic!("an ellipse");
    };
    assert!((frame.rx.as_mm() - 17.5).abs() < 1e-6);
    assert!((frame.ry.as_mm() - 10.0).abs() < 1e-9);
    assert!((frame.center.x - 7.5).abs() < 1e-6);
}

/// Was `PolygonStarTool` AC13: a corner handle scales a star uniformly about
/// its centre, keeping point count and ratio (outer radius 10 becomes 20).
#[test]
fn ported_ac13_a_corner_handle_scales_a_star_uniformly_keeping_count_and_ratio() {
    let document = Document::new(1);
    let id = document.create_star(
        StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0)),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let mut rig = Zoomed::new(document, id);
    let corner = rig.handle(EditHandle::Resize(ResizeDirection::Se));
    // Along the corner's diagonal by sqrt(2) times the radius change.
    let d = 10.0;
    rig.drag(corner, Vec2::new(d, d));
    let Shape::Star {
        frame,
        point_count,
        inner_ratio,
    } = rig.shape()
    else {
        panic!("a star");
    };
    assert!(
        (frame.radius.as_mm() - 20.0).abs() < 1e-6,
        "{}",
        frame.radius.as_mm()
    );
    assert_eq!(point_count.get(), 5);
    assert!((inner_ratio.get() - 0.5).abs() < 1e-9);
}

/// Was `PolygonStarTool` AC14: the inner-radius handle changes the ratio and
/// keeps the outer radius (a 1 mm push outward raises the ratio).
#[test]
fn ported_ac14_the_inner_radius_handle_changes_the_ratio_and_keeps_the_outer_radius() {
    let document = Document::new(1);
    let id = document.create_star(
        StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0)),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let mut rig = Zoomed::new(document, id);
    let knob = rig.handle(EditHandle::Param(ParamHandle::InnerRadius));
    let outward = pt(0.0, 0.0).vector_to(knob).normalized_to(1.0);
    assert_eq!(rig.drag(knob, outward), SelectPointerDownOutcome::Handle);
    let Shape::Star {
        frame, inner_ratio, ..
    } = rig.shape()
    else {
        panic!("a star");
    };
    assert!((frame.radius.as_mm() - 10.0).abs() < 1e-9);
    assert!(inner_ratio.get() > 0.5);
}

/// Was `PolygonStarTool` AC15: a point-count change from the bar updates the
/// selected star and keeps its size and ratio.
#[test]
fn ported_ac15_a_point_count_change_keeps_size_and_ratio() {
    let document = Document::new(1);
    let id = document.create_star(
        StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0)),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let mut rig = Zoomed::new(document, id);
    let objects = rig.objects();
    rig.tool
        .commit_bar_value(
            &rig.document,
            &objects,
            &rig.selection,
            curvyo_ui_core::ParamValue::PointCount(PointCount::new(12).unwrap()),
        )
        .unwrap();
    let Shape::Star {
        frame,
        point_count,
        inner_ratio,
    } = rig.shape()
    else {
        panic!("a star");
    };
    assert_eq!(point_count.get(), 12);
    assert!((frame.radius.as_mm() - 10.0).abs() < 1e-9);
    assert!((inner_ratio.get() - 0.4).abs() < 1e-9);
}

/// Replaces `hit_test_handle_ignores_non_draggable_echo_handles`: on a rounded
/// rectangle all four radius handles are real, draggable handles.
#[test]
fn ported_all_four_radius_handles_of_a_rounded_rectangle_are_draggable() {
    for corner in Corner::ALL {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(40.0, 40.0)));
        document
            .set_corner_radius(&[id], Length::from_mm(8.0))
            .unwrap();
        let mut rig = Zoomed::new(document, id);
        let knob = rig.handle(EditHandle::Param(ParamHandle::CornerRadius(corner)));
        let inward = knob.vector_to(pt(20.0, 20.0)).normalized_to(1.0);
        assert_eq!(
            rig.drag(knob, inward.scaled(0.5)),
            SelectPointerDownOutcome::Handle,
            "{corner:?}"
        );
        let Shape::Rect { corner_radius, .. } = rig.shape() else {
            panic!("a rectangle");
        };
        assert!(
            corner_radius.as_mm() > 8.0,
            "{corner:?}: dragging inward grows it"
        );
    }
}
