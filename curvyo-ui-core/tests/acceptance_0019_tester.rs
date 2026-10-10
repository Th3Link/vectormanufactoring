//! Independent tester acceptance tests for the group box and its handles
//! (`specs/0019-multi-object-transform/specification.md`, Parts A, B and E),
//! at the `curvyo-ui-core` level: the box, the handle sets per tier and per
//! kind of selection, degenerate boxes, the hit rule and the hint cause line.
//! Written from the specification before the implementation was read. Expected
//! positions come from the design system's pixel layout (rotate 32 px out on
//! the diagonal, skew 16 px out, resize at corners and edge midpoints).

// Test code: byte buffers are compared with `assert!(a == b, "msg")` on purpose
// (a failing `assert_eq!` would print the whole snapshot), and the arithmetic is
// written the way the specification states it.
#![allow(
    clippy::manual_assert_eq,
    clippy::manual_midpoint,
    clippy::collapsible_if
)]
#![allow(clippy::unneeded_wildcard_pattern, clippy::unreadable_literal)]
#![allow(clippy::used_underscore_binding, clippy::useless_conversion)]
#![allow(clippy::suboptimal_flops, clippy::imprecise_flops)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss, missing_docs)]
#![allow(clippy::many_single_char_names, clippy::doc_markdown)]
#![allow(clippy::items_after_statements)]

use std::f64::consts::{FRAC_PI_2, SQRT_2};

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot,
    Point, PointCount, RectBounds, StarFrame,
};
use curvyo_ui_core::{
    EditHandle, GroupBoxShape, ObjectSelection, ResizeDirection, SelectTool, Side,
    TransformHandleTolerances,
};

/// Pixels per millimetre of every scene: 4, so a size in px is a size in mm * 4.
const K: f64 = 4.0;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn tol() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(K)
}

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn rect(d: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    })
}

fn line(d: &Document, n: u64, a: (f64, f64), b: (f64, f64)) -> NodeId {
    d.create_path(&[anchor(n, a.0, a.1), anchor(n + 1, b.0, b.1)], false)
}

fn rotate(d: &Document, id: NodeId, about: Point, radians: f64) {
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(about, Angle::from_radians(radians)))
        .unwrap();
}

fn star(d: &Document, cx: f64, cy: f64, r: f64) -> NodeId {
    d.create_star(
        StarFrame {
            center: pt(cx, cy),
            radius: Length::from_mm(r),
            angle: Angle::from_radians(-FRAC_PI_2),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    )
}

fn polygon(d: &Document, cx: f64, cy: f64, r: f64) -> NodeId {
    d.create_polygon(
        StarFrame {
            center: pt(cx, cy),
            radius: Length::from_mm(r),
            angle: Angle::from_radians(-FRAC_PI_2),
        },
        PointCount::new(6).unwrap(),
    )
}

fn circle(d: &Document, cx: f64, cy: f64, r: f64) -> NodeId {
    d.create_ellipse(EllipseFrame {
        center: pt(cx, cy),
        rx: Length::from_mm(r),
        ry: Length::from_mm(r),
    })
}

fn snapshots(d: &Document) -> Vec<ObjectSnapshot> {
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect()
}

fn selection(d: &Document) -> ObjectSelection {
    let mut s = ObjectSelection::new();
    s.set(&d.object_ids());
    s
}

fn handles(d: &Document, shift: bool) -> Vec<(EditHandle, Point)> {
    SelectTool::group_handle_positions(&snapshots(d), &selection(d), tol(), shift)
}

fn has(h: &[(EditHandle, Point)], which: EditHandle) -> bool {
    h.iter().any(|(e, _)| *e == which)
}

fn at(h: &[(EditHandle, Point)], which: EditHandle) -> Point {
    h.iter()
        .find(|(e, _)| *e == which)
        .unwrap_or_else(|| panic!("{which:?} is not in the set"))
        .1
}

fn close(a: Point, x: f64, y: f64) -> bool {
    (a.x - x).abs() < 1e-6 && (a.y - y).abs() < 1e-6
}

use ResizeDirection::{E, N, Ne, Nw, S, Se, Sw, W};

const CORNER_RESIZE: [ResizeDirection; 4] = [Nw, Ne, Se, Sw];
const EDGE_RESIZE: [ResizeDirection; 4] = [N, E, S, W];

fn count_kind(h: &[(EditHandle, Point)], f: impl Fn(EditHandle) -> bool) -> usize {
    h.iter().filter(|(e, _)| f(*e)).count()
}

/// Two closed paths: a 30 x 10 mm group box (120 x 40 px).
fn two_paths(x0: f64, y0: f64, w: f64, h: f64) -> Document {
    let d = Document::new(1);
    for (n, ox) in [(1_u64, x0), (11, x0 + w - 10.0)] {
        let _ = d.create_path(
            &[
                anchor(n, ox, y0),
                anchor(n + 1, ox + 10.0, y0),
                anchor(n + 2, ox + 10.0, y0 + h),
                anchor(n + 3, ox, y0 + h),
            ],
            true,
        );
    }
    d
}

// ---------------------------------------------------------------------
// Criterion 1 to 3, 6: the box
// ---------------------------------------------------------------------

/// Criterion 1: the spec's example, to 0.01 mm: x -1.16..45.00, y -4.33..14.33.
#[test]
fn ac1_example_group_box() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 20.0, 10.0);
    rotate(&d, r, pt(10.0, 5.0), 30.0_f64.to_radians());
    let _ = circle(&d, 40.0, 5.0, 5.0);
    let g = SelectTool::group_of(&snapshots(&d), &selection(&d)).expect("a group");
    let b = g.bounds();
    assert!((b.min.x - -1.16).abs() < 0.01, "x0 {}", b.min.x);
    assert!((b.max.x - 45.0).abs() < 0.01, "x1 {}", b.max.x);
    assert!((b.min.y - -4.33).abs() < 0.01, "y0 {}", b.min.y);
    assert!((b.max.y - 14.33).abs() < 0.01, "y1 {}", b.max.y);
    assert_eq!(b.angle.as_radians(), 0.0, "axis-aligned");
    assert_eq!(g.count(), 2);
}

/// Criterion 6: fewer than two objects is no group.
#[test]
fn ac6_one_object_or_none_has_no_group() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let _ = rect(&d, 20.0, 0.0, 10.0, 10.0);
    let mut one = ObjectSelection::new();
    one.select_single(a);
    assert!(SelectTool::group_of(&snapshots(&d), &one).is_none());
    assert!(SelectTool::group_of(&snapshots(&d), &ObjectSelection::new()).is_none());
    assert_eq!(
        SelectTool::group_handle_positions(&snapshots(&d), &one, tol(), false).len(),
        0
    );
}

/// Criterion 3/1: a polygon and a star contribute their outline, a path its
/// curve (a bulging curve extends past its anchors).
#[test]
fn ac1_curve_accurate_bounds_for_a_path() {
    let d = Document::new(1);
    let mut a = anchor(1, 0.0, 0.0);
    a.handle_out = curvyo_document_core::Vec2::new(0.0, -30.0);
    let mut b = anchor(2, 10.0, 0.0);
    b.handle_in = curvyo_document_core::Vec2::new(0.0, -30.0);
    let _ = d.create_path(&[a, b], false);
    let _ = rect(&d, 40.0, 0.0, 10.0, 10.0);
    let g = SelectTool::group_of(&snapshots(&d), &selection(&d)).unwrap();
    // The cubic (0,0) (0,-30) (10,-30) (10,0) peaks at y = -22.5 (t = 0.5).
    assert!(
        (g.bounds().min.y - -22.5).abs() < 1e-3,
        "{}",
        g.bounds().min.y
    );
    assert!((g.bounds().min.x - 0.0).abs() < 1e-6);
    assert!((g.bounds().max.x - 50.0).abs() < 1e-6);
}

// ---------------------------------------------------------------------
// Criteria 9, 10: the handle set per tier
// ---------------------------------------------------------------------

/// Criterion 9/10: two paths, s = 40 px: the four corner resize, the four
/// edge resize, the four corner rotate, four skew; no centre (s < 48), and no
/// side rotate without Shift. With Shift the four side rotate handles join.
#[test]
fn ac9_handle_set_of_two_paths_in_the_edge_tier() {
    let d = two_paths(0.0, 0.0, 30.0, 10.0);
    let h = handles(&d, false);
    for dir in CORNER_RESIZE {
        assert!(has(&h, EditHandle::Resize(dir)), "{dir:?}");
        assert!(has(&h, EditHandle::Rotate(dir)), "rotate {dir:?}");
    }
    for dir in EDGE_RESIZE {
        assert!(has(&h, EditHandle::Resize(dir)), "{dir:?}");
        assert!(
            !has(&h, EditHandle::Rotate(dir)),
            "no side rotate without Shift"
        );
    }
    for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
        assert!(has(&h, EditHandle::Skew(side)), "{side:?}");
    }
    assert!(!has(&h, EditHandle::Move), "s = 40 px: no centre handle");
    assert_eq!(count_kind(&h, |e| matches!(e, EditHandle::Param(_))), 0);
    let hs = handles(&d, true);
    for dir in EDGE_RESIZE {
        assert!(
            has(&hs, EditHandle::Rotate(dir)),
            "side rotate {dir:?} with Shift"
        );
    }
}

/// Positions of the group handles (design system, in px / K): corners and edge
/// midpoints, rotate 32 px out, skew 16 px out.
#[test]
fn ac9_handle_positions() {
    let d = two_paths(10.0, 20.0, 30.0, 10.0); // box (10,20)-(40,30)
    let h = handles(&d, true);
    let diag = 32.0 / SQRT_2 / K;
    assert!(close(at(&h, EditHandle::Resize(Nw)), 10.0, 20.0));
    assert!(close(at(&h, EditHandle::Resize(Se)), 40.0, 30.0));
    assert!(close(at(&h, EditHandle::Resize(N)), 25.0, 20.0));
    assert!(close(at(&h, EditHandle::Resize(E)), 40.0, 25.0));
    assert!(close(
        at(&h, EditHandle::Rotate(Nw)),
        10.0 - diag,
        20.0 - diag
    ));
    assert!(close(
        at(&h, EditHandle::Rotate(Se)),
        40.0 + diag,
        30.0 + diag
    ));
    assert!(close(at(&h, EditHandle::Rotate(N)), 25.0, 20.0 - 32.0 / K));
    assert!(close(at(&h, EditHandle::Rotate(W)), 10.0 - 32.0 / K, 25.0));
    assert!(close(
        at(&h, EditHandle::Skew(Side::Top)),
        25.0,
        20.0 - 16.0 / K
    ));
    assert!(close(
        at(&h, EditHandle::Skew(Side::Right)),
        40.0 + 16.0 / K,
        25.0
    ));
}

/// Criterion 10: under 24 px only corner resize and corner rotate are drawn,
/// the edge resize handles stay hit-testable; from 48 px the centre handle.
#[test]
fn ac10_tiers_on_the_shorter_side() {
    // 80 x 20 px: s = 20 px.
    let d = two_paths(0.0, 0.0, 20.0, 5.0);
    let objects = snapshots(&d);
    let sel = selection(&d);
    let g = SelectTool::group_of(&objects, &sel).unwrap();
    for dir in CORNER_RESIZE {
        assert!(SelectTool::group_handle_is_drawn(
            EditHandle::Resize(dir),
            &g,
            &tol()
        ));
    }
    for dir in EDGE_RESIZE {
        assert!(
            !SelectTool::group_handle_is_drawn(EditHandle::Resize(dir), &g, &tol()),
            "{dir:?} not drawn under 24 px"
        );
    }
    // Hit-testable although not drawn: the pointer on the north edge midpoint.
    let hit = SelectTool::group_handle_at(&objects, &sel, pt(10.0, 0.0), tol(), false, true)
        .map(|(_, e)| e);
    assert_eq!(hit, Some(EditHandle::Resize(N)));
    // No centre handle.
    assert!(!has(&handles(&d, false), EditHandle::Move));

    // s = 47 px: no centre; s = 48 px: centre. Box 30 x 11.75 mm / 12 mm.
    let d47 = two_paths(0.0, 0.0, 30.0, 11.75);
    assert!(!has(&handles(&d47, false), EditHandle::Move));
    let d48 = two_paths(0.0, 0.0, 30.0, 12.0);
    let h = handles(&d48, false);
    assert!(close(at(&h, EditHandle::Move), 15.0, 6.0));
}

/// Criterion 11: the centre handle is tested last and only inside its hover
/// region; without `include_move` it is not a hit at all.
#[test]
fn ac11_centre_handle_hit_region() {
    let d = two_paths(0.0, 0.0, 40.0, 20.0); // 160 x 80 px
    let objects = snapshots(&d);
    let sel = selection(&d);
    let hit = |p: Point, include| {
        SelectTool::group_handle_at(&objects, &sel, p, tol(), false, include).map(|(_, e)| e)
    };
    assert_eq!(hit(pt(20.0, 10.0), true), Some(EditHandle::Move));
    assert_eq!(hit(pt(20.0, 10.0), false), None);
    // Far from the centre inside the box and away from all handles: no hit.
    assert_eq!(hit(pt(10.0, 10.0), true), None);
    // The hover region is min(12 px, s/4) = 12 px = 3 mm.
    assert_eq!(hit(pt(22.5, 10.0), true), Some(EditHandle::Move));
    assert_eq!(hit(pt(24.0, 10.0), true), None);
}

/// Criterion 11: the corner resize handle wins over a rotate handle nearby and
/// the nearest centre decides: a point 8 px diagonal out is still the resize.
#[test]
fn ac11_nearest_centre_between_resize_and_rotate() {
    let d = two_paths(0.0, 0.0, 40.0, 20.0);
    let objects = snapshots(&d);
    let sel = selection(&d);
    let hit = |p: Point| {
        SelectTool::group_handle_at(&objects, &sel, p, tol(), false, true).map(|(_, e)| e)
    };
    assert_eq!(
        hit(pt(40.0 + 2.0 / K, 20.0 + 2.0 / K)),
        Some(EditHandle::Resize(Se))
    );
    let off = 32.0 / SQRT_2 / K;
    assert_eq!(
        hit(pt(40.0 + off, 20.0 + off)),
        Some(EditHandle::Rotate(Se))
    );
    // Outside every cap: no group handle.
    assert_eq!(hit(pt(70.0, 50.0)), None);
}

// ---------------------------------------------------------------------
// Criteria 12, 13: degenerate boxes
// ---------------------------------------------------------------------

/// Criterion 12: two horizontal lines at one height, 80 px wide: height 0.
/// Only the E and W edge resize handles remain, no corner resize; the centre
/// handle is drawn; the four corner rotate handles stand at the notional
/// square's diagonals; the top and bottom skew arrows are gone.
#[test]
fn ac12_box_flat_in_y() {
    let d = Document::new(1);
    let _ = line(&d, 1, (0.0, 10.0), (8.0, 10.0));
    let _ = line(&d, 11, (12.0, 10.0), (20.0, 10.0)); // 20 mm = 80 px wide
    let g = SelectTool::group_of(&snapshots(&d), &selection(&d)).unwrap();
    assert_eq!(g.shape(), GroupBoxShape::Line);
    let h = handles(&d, false);
    for dir in CORNER_RESIZE {
        assert!(
            !has(&h, EditHandle::Resize(dir)),
            "no corner resize {dir:?}"
        );
    }
    assert!(has(&h, EditHandle::Resize(E)) && has(&h, EditHandle::Resize(W)));
    assert!(!has(&h, EditHandle::Resize(N)) && !has(&h, EditHandle::Resize(S)));
    assert!(has(&h, EditHandle::Move), "the centre handle is drawn");
    let diag = 32.0 / SQRT_2 / K;
    assert!(close(at(&h, EditHandle::Rotate(Nw)), -diag, 10.0 - diag));
    assert!(close(
        at(&h, EditHandle::Rotate(Se)),
        20.0 + diag,
        10.0 + diag
    ));
    assert!(!has(&h, EditHandle::Skew(Side::Top)));
    assert!(!has(&h, EditHandle::Skew(Side::Bottom)));
}

/// Criterion 12: the same for a box flat in x (vertical lines): only N and S.
#[test]
fn ac12_box_flat_in_x() {
    let d = Document::new(1);
    let _ = line(&d, 1, (5.0, 0.0), (5.0, 8.0));
    let _ = line(&d, 11, (5.0, 12.0), (5.0, 20.0));
    let g = SelectTool::group_of(&snapshots(&d), &selection(&d)).unwrap();
    assert_eq!(g.shape(), GroupBoxShape::Line);
    let h = handles(&d, false);
    assert!(has(&h, EditHandle::Resize(N)) && has(&h, EditHandle::Resize(S)));
    assert!(!has(&h, EditHandle::Resize(E)) && !has(&h, EditHandle::Resize(W)));
    for dir in CORNER_RESIZE {
        assert!(!has(&h, EditHandle::Resize(dir)));
    }
    assert!(!has(&h, EditHandle::Skew(Side::Left)));
    assert!(!has(&h, EditHandle::Skew(Side::Right)));
}

/// Criterion 13: all selected objects are one point: no handle at all and the
/// shape is a point.
#[test]
fn ac13_a_single_point_has_no_handle() {
    let d = Document::new(1);
    let _ = line(&d, 1, (7.0, 7.0), (7.0, 7.0));
    let _ = line(&d, 11, (7.0, 7.0), (7.0, 7.0));
    let g = SelectTool::group_of(&snapshots(&d), &selection(&d)).unwrap();
    assert_eq!(g.shape(), GroupBoxShape::Point);
    assert_eq!(g.side_mm(), 0.0);
    assert_eq!(handles(&d, false).len(), 0);
    assert_eq!(handles(&d, true).len(), 0);
    // And no hit anywhere near.
    let objects = snapshots(&d);
    let sel = selection(&d);
    assert!(
        SelectTool::group_handle_at(&objects, &sel, pt(7.0, 7.0), tol(), false, true).is_none()
    );
}

/// Criterion 8: a degenerate axis is the geometric tolerance. A box 1e-9 wide
/// is flat; one 0.01 mm wide is not.
#[test]
fn ac8_degenerate_below_the_geometric_tolerance() {
    let d = Document::new(1);
    let _ = line(&d, 1, (5.0, 0.0), (5.0 + 1e-9, 8.0));
    let _ = line(&d, 11, (5.0, 12.0), (5.0, 20.0));
    let g = SelectTool::group_of(&snapshots(&d), &selection(&d)).unwrap();
    assert_eq!(g.shape(), GroupBoxShape::Line);
    let d2 = Document::new(1);
    let _ = line(&d2, 1, (5.0, 0.0), (5.01, 8.0));
    let _ = line(&d2, 11, (5.0, 12.0), (5.0, 20.0));
    let g2 = SelectTool::group_of(&snapshots(&d2), &selection(&d2)).unwrap();
    assert_eq!(g2.shape(), GroupBoxShape::Box);
}

// ---------------------------------------------------------------------
// Criterion 21, 40: kinds
// ---------------------------------------------------------------------

/// Criterion 40: any rectangle, ellipse, polygon or star removes every skew
/// handle (hidden, not inert); a path-only selection keeps all four.
#[test]
fn ac40_primitives_in_the_selection_hide_skew() {
    let skew = |d: &Document| {
        handles(d, true)
            .iter()
            .filter(|(e, _)| matches!(e, EditHandle::Skew(_)))
            .count()
    };
    let paths = two_paths(0.0, 0.0, 40.0, 20.0);
    assert_eq!(skew(&paths), 4);
    for kind in 0..4 {
        let d = two_paths(0.0, 0.0, 40.0, 20.0);
        match kind {
            0 => {
                let _ = rect(&d, 5.0, 5.0, 5.0, 5.0);
            }
            1 => {
                let _ = circle(&d, 20.0, 10.0, 3.0);
            }
            2 => {
                let _ = polygon(&d, 20.0, 10.0, 3.0);
            }
            _ => {
                let _ = star(&d, 20.0, 10.0, 3.0);
            }
        }
        assert_eq!(skew(&d), 0, "kind {kind}");
    }
}

/// Criterion 18 (replacing the removed criterion 21): every selection shows all four
/// edge resize handles and all four corners, whatever kinds it holds; the kinds a
/// stretch converts change nothing about the handles.
#[test]
fn ac18_every_selection_has_all_eight_resize_handles() {
    let edge_count = |d: &Document| {
        handles(d, false)
            .iter()
            .filter(|(e, _)| matches!(e, EditHandle::Resize(dir) if EDGE_RESIZE.contains(dir)))
            .count()
    };
    let corner_count = |d: &Document| {
        handles(d, false)
            .iter()
            .filter(|(e, _)| matches!(e, EditHandle::Resize(dir) if CORNER_RESIZE.contains(dir)))
            .count()
    };
    type Add = fn(&Document);
    let cases: [(&str, Add); 7] = [
        ("polygon", |d| {
            let _ = polygon(d, 20.0, 10.0, 3.0);
        }),
        ("star", |d| {
            let _ = star(d, 20.0, 10.0, 3.0);
        }),
        ("rotated rectangle", |d| {
            let r = rect(d, 15.0, 5.0, 10.0, 6.0);
            rotate(d, r, pt(20.0, 8.0), 0.3);
        }),
        ("rotated ellipse", |d| {
            let e = d.create_ellipse(EllipseFrame {
                center: pt(20.0, 10.0),
                rx: Length::from_mm(6.0),
                ry: Length::from_mm(3.0),
            });
            rotate(d, e, pt(20.0, 10.0), 0.3);
        }),
        ("circle", |d| {
            let c = circle(d, 20.0, 10.0, 4.0);
            rotate(d, c, pt(20.0, 10.0), 30.0_f64.to_radians());
        }),
        ("aligned rectangle", |d| {
            let r = rect(d, 15.0, 5.0, 10.0, 6.0);
            rotate(d, r, pt(20.0, 8.0), FRAC_PI_2);
        }),
        ("plain rectangle", |d| {
            let _ = rect(d, 15.0, 5.0, 10.0, 6.0);
        }),
    ];
    for (name, add) in cases {
        let d = two_paths(0.0, 0.0, 40.0, 20.0);
        add(&d);
        assert_eq!(edge_count(&d), 4, "{name}");
        assert_eq!(corner_count(&d), 4, "{name}");
    }
}

/// Criteria 53 and 55 (replacing the cause line of the removed criterion 21): the
/// shapes a stretch would convert are counted by kind in the order polygon, star,
/// rotated rectangle, rotated ellipse; a circle never counts.
#[test]
fn ac55_converting_kinds_are_counted() {
    let counts_of = |d: &Document| {
        SelectTool::group_of(&snapshots(d), &selection(d))
            .unwrap()
            .converting()
            .as_array()
    };
    let d = two_paths(0.0, 0.0, 40.0, 20.0);
    assert_eq!(counts_of(&d), [0, 0, 0, 0]);
    let _ = star(&d, 10.0, 10.0, 3.0);
    let r = rect(&d, 15.0, 5.0, 10.0, 6.0);
    rotate(&d, r, pt(20.0, 8.0), 0.3);
    assert_eq!(counts_of(&d), [0, 1, 1, 0]);
    let _ = polygon(&d, 30.0, 10.0, 3.0);
    assert_eq!(counts_of(&d), [1, 1, 1, 0]);
}

/// Criterion 14: a handle that is not drawn has no hit area: the skew handle of a
/// mixed selection is not there; the north edge handle is (every selection has the
/// edge handles, criterion 18).
#[test]
fn ac14_a_handle_that_is_not_drawn_has_no_hit_area() {
    let d = two_paths(0.0, 0.0, 40.0, 20.0);
    let _ = star(&d, 20.0, 10.0, 3.0);
    let objects = snapshots(&d);
    let sel = selection(&d);
    let hit = SelectTool::group_handle_at(&objects, &sel, pt(20.0, 0.0), tol(), false, true)
        .map(|(_, e)| e);
    assert_eq!(hit, Some(EditHandle::Resize(N)));
    // The skew handle of a mixed selection is not there either.
    let hit = SelectTool::group_handle_at(&objects, &sel, pt(20.0, -4.0), tol(), false, true)
        .map(|(_, e)| e);
    assert!(!matches!(hit, Some(EditHandle::Skew(_))));
}

/// Criterion 25: the pivot of a rotate by the opposite corner (Shift) and the
/// box centre (no Shift); a side rotate with Shift pivots at the opposite side.
#[test]
fn ac25_hover_pivots() {
    let d = two_paths(10.0, 20.0, 30.0, 10.0); // (10,20)-(40,30)
    let objects = snapshots(&d);
    let sel = selection(&d);
    let g = SelectTool::group_of(&objects, &sel).unwrap();
    let p = |h: EditHandle, shift| SelectTool::group_hover_pivot(&g, h, shift);
    let c = p(EditHandle::Rotate(Se), false).unwrap();
    assert!(close(c, 25.0, 25.0));
    let c = p(EditHandle::Rotate(Se), true).unwrap();
    assert!(close(c, 10.0, 20.0), "opposite corner");
    let c = p(EditHandle::Rotate(E), true).unwrap();
    assert!(close(c, 10.0, 25.0), "opposite side midpoint");
    // A scale handle holds the opposite corner / side; the centre with Shift.
    let c = p(EditHandle::Resize(Se), false).unwrap();
    assert!(close(c, 10.0, 20.0));
    let c = p(EditHandle::Resize(Se), true).unwrap();
    assert!(close(c, 25.0, 25.0));
    let c = p(EditHandle::Resize(N), false).unwrap();
    assert!(close(c, 25.0, 30.0));
    assert!(p(EditHandle::Move, false).is_none());
}
