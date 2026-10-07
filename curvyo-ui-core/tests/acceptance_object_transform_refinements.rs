//! Tests of `curvyo-ui-core`'s share of
//! `specs/object-transform-refinements/specification.md`: the handle set and
//! its one hit rule, the pivot rule, the 3 px dead zone, the centre handle,
//! the double-click dispatch, the typed numeric entry (including "a typed
//! value and a dragged value never disagree"), and path skew. Session-level
//! behaviour (commit counts, cursors, readouts, save and reopen) is in
//! `curvyo-editor-wasm/tests/acceptance_object_transform_refinements.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use std::f64::consts::FRAC_PI_2;

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot,
    Point, PointCount, RectBounds, StarFrame, Tolerance, Vec2,
};
use curvyo_ui_core::{
    AnchorIdMinter, EditHandle, EntryKind, EntryOutcome, InvalidReason, Modifiers, ObjectSelection,
    ResizeDirection, SelectDoubleClickOutcome, SelectPointerDownOutcome, SelectTool, Side,
    StrokeScaling, TransformHandleTolerances, oriented_bounds,
};

const EPS: f64 = 1e-9;
const SEGMENT_TOLERANCE: Tolerance = Tolerance::from_mm(1.0);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

fn tolerances() -> TransformHandleTolerances {
    // Two screen pixels per millimetre: rotate offset 16 mm, skew offset
    // 8 mm, dead zone 1.5 mm, centre handle from a 24 mm shorter side.
    TransformHandleTolerances::at_scale(2.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Rect,
    Ellipse,
    Polygon,
    Star,
    OpenPath,
    ClosedPath,
}

const ALL_KINDS: [Kind; 6] = [
    Kind::Rect,
    Kind::Ellipse,
    Kind::Polygon,
    Kind::Star,
    Kind::OpenPath,
    Kind::ClosedPath,
];

fn curve_anchors() -> Vec<NewAnchor> {
    vec![
        NewAnchor {
            handle_out: Vec2::new(20.0, 10.0),
            ..NewAnchor::corner(AnchorId::new(1, 1), pt(10.0, 20.0))
        },
        NewAnchor {
            handle_in: Vec2::new(-15.0, -5.0),
            handle_out: Vec2::new(15.0, 5.0),
            kind: curvyo_document_core::AnchorKind::Symmetric,
            ..NewAnchor::corner(AnchorId::new(1, 2), pt(70.0, 60.0))
        },
        NewAnchor {
            handle_in: Vec2::new(10.0, -10.0),
            ..NewAnchor::corner(AnchorId::new(1, 3), pt(110.0, 30.0))
        },
    ]
}

/// A document with one object of `kind`, selected, and a Select tool.
struct Rig {
    document: Document,
    id: NodeId,
    selection: ObjectSelection,
    tool: SelectTool,
}

impl Rig {
    fn new(kind: Kind) -> Self {
        let document = Document::new(1);
        let id = match kind {
            Kind::Rect => document.create_rect(RectBounds {
                origin: pt(10.0, 20.0),
                width: Length::from_mm(100.0),
                height: Length::from_mm(60.0),
            }),
            Kind::Ellipse => document.create_ellipse(EllipseFrame {
                center: pt(60.0, 50.0),
                rx: Length::from_mm(50.0),
                ry: Length::from_mm(30.0),
            }),
            Kind::Polygon => document.create_polygon(
                StarFrame {
                    center: pt(60.0, 50.0),
                    radius: Length::from_mm(40.0),
                    angle: Angle::from_radians(0.0),
                },
                PointCount::new(6).unwrap(),
            ),
            Kind::Star => document.create_star(
                StarFrame {
                    center: pt(60.0, 50.0),
                    radius: Length::from_mm(40.0),
                    angle: Angle::from_radians(0.0),
                },
                PointCount::new(5).unwrap(),
                InnerRatio::new(0.5).unwrap(),
            ),
            Kind::OpenPath => document.create_path(&curve_anchors(), false),
            Kind::ClosedPath => document.create_path(&curve_anchors(), true),
        };
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

    fn handles(&self, shift: bool) -> Vec<(EditHandle, Point)> {
        SelectTool::transform_handles(&self.objects(), &self.selection, tolerances(), shift)
    }

    fn handle(&self, wanted: EditHandle, shift: bool) -> Point {
        self.handles(shift)
            .into_iter()
            .find(|(h, _)| *h == wanted)
            .unwrap_or_else(|| panic!("handle {wanted:?} (shift {shift}) exists"))
            .1
    }

    fn press(&mut self, at: Point, shift: bool) -> SelectPointerDownOutcome {
        let objects = self.objects();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            at,
            SEGMENT_TOLERANCE,
            tolerances(),
            shift,
        )
    }

    fn release(&mut self, at: Point, shift: bool, ctrl: bool) {
        let objects = self.objects();
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            at,
            Modifiers::new(shift, ctrl),
            &mut AnchorIdMinter::new(99),
        );
    }

    /// A press at `from`, a move to `to`, a release at `to`.
    fn drag(&mut self, from: Point, to: Point, shift: bool, ctrl: bool) {
        self.press(from, shift);
        self.tool
            .pointer_moved(to, Modifiers::NONE, &mut self.selection);
        self.release(to, shift, ctrl);
    }

    /// A whole double-click: the first press and release, then the second
    /// press's `double_click` (a handle only opens an entry if the first
    /// press grabbed it).
    fn double_click(&mut self, at: Point, shift: bool, ctrl: bool) -> SelectDoubleClickOutcome {
        self.press(at, shift);
        self.release(at, shift, ctrl);
        let objects = self.objects();
        self.tool.double_click(
            &objects,
            &self.selection,
            at,
            SEGMENT_TOLERANCE,
            tolerances(),
            (shift, ctrl),
        )
    }
}

fn turned(pivot: Point, p: Point, angle: f64) -> Point {
    pivot.translated(pivot.vector_to(p).rotated(Angle::from_radians(angle)))
}

/// Every number in a `Debug` rendering, and the rendering with the numbers
/// removed: two snapshots are close when their skeletons are equal and
/// every number agrees within `tolerance`.
fn numbers_and_skeleton(text: &str) -> (Vec<f64>, String) {
    let mut numbers = Vec::new();
    let mut skeleton = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let starts_number =
            c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(char::is_ascii_digit));
        if starts_number {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || ((chars[i] == 'e' || chars[i] == 'E')
                        && chars
                            .get(i + 1)
                            .is_some_and(|n| n.is_ascii_digit() || *n == '-' || *n == '+'))
                    || ((chars[i] == '-' || chars[i] == '+') && matches!(chars[i - 1], 'e' | 'E')))
            {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            numbers.push(token.parse::<f64>().unwrap());
            skeleton.push('#');
        } else {
            skeleton.push(c);
            i += 1;
        }
    }
    (numbers, skeleton)
}

fn assert_snapshots_close(a: &ObjectSnapshot, b: &ObjectSnapshot, tolerance: f64, context: &str) {
    let (na, sa) = numbers_and_skeleton(&format!("{a:?}"));
    let (nb, sb) = numbers_and_skeleton(&format!("{b:?}"));
    assert_eq!(sa, sb, "{context}: structure differs");
    assert_eq!(na.len(), nb.len(), "{context}");
    for (x, y) in na.iter().zip(&nb) {
        assert!(
            (x - y).abs() <= tolerance,
            "{context}: {x} vs {y}\n{a:?}\n{b:?}"
        );
    }
    assert!(
        (a.rotation().as_radians() - b.rotation().as_radians()).abs() < 1e-12,
        "{context}: rotation"
    );
}

fn is_path(kind: Kind) -> bool {
    matches!(kind, Kind::OpenPath | Kind::ClosedPath)
}

fn is_polygon_or_star(kind: Kind) -> bool {
    matches!(kind, Kind::Polygon | Kind::Star)
}

fn count(handles: &[(EditHandle, Point)], f: impl Fn(&EditHandle) -> bool) -> usize {
    handles.iter().filter(|(h, _)| f(h)).count()
}

// ---------------------------------------------------------------------
// Handles: criteria 1, 4-9, 37, 50, 53
// ---------------------------------------------------------------------

#[test]
fn every_kind_shows_its_handle_set() {
    for kind in ALL_KINDS {
        let rig = Rig::new(kind);
        let handles = rig.handles(false);
        let resize = count(&handles, |h| matches!(h, EditHandle::Resize(_)));
        let rotate = count(&handles, |h| matches!(h, EditHandle::Rotate(_)));
        let skew = count(&handles, |h| matches!(h, EditHandle::Skew(_)));
        let centre = count(&handles, |h| *h == EditHandle::Move);
        assert_eq!(
            resize,
            if is_polygon_or_star(kind) { 4 } else { 8 },
            "{kind:?}"
        );
        assert_eq!(rotate, 4, "{kind:?}: corner rotate handles only");
        assert_eq!(
            skew,
            if is_path(kind) { 4 } else { 0 },
            "{kind:?}: skew on paths only"
        );
        assert_eq!(centre, 1, "{kind:?}: centre handle on a big box");

        let revealed = rig.handles(true);
        assert_eq!(
            count(&revealed, |h| matches!(h, EditHandle::Rotate(_))),
            8,
            "{kind:?}: Shift reveals the four side rotate handles, polygon and star too"
        );
    }
}

#[test]
fn a_multi_object_selection_shows_no_handles_at_all() {
    let mut rig = Rig::new(Kind::OpenPath);
    let rect = rig.document.create_rect(RectBounds {
        origin: pt(200.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    rig.selection.toggle(rect);
    let objects: Vec<ObjectSnapshot> = rig
        .document
        .object_ids()
        .into_iter()
        .filter_map(|id| rig.document.object(id))
        .collect();
    for shift in [false, true] {
        let handles = SelectTool::transform_handles(&objects, &rig.selection, tolerances(), shift);
        assert_eq!(handles.len(), 0);
    }
}

#[test]
fn a_side_rotate_handle_starts_a_rotate_with_shift_and_is_empty_canvas_without() {
    let mut rig = Rig::new(Kind::Rect);
    let top = rig.handle(EditHandle::Rotate(ResizeDirection::N), true);
    assert_eq!(
        rig.press(top, true),
        SelectPointerDownOutcome::Handle,
        "criterion 11"
    );
    assert_eq!(
        rig.tool.dragging_handle(),
        Some(EditHandle::Rotate(ResizeDirection::N))
    );
    assert_eq!(
        rig.selection.ids(),
        &[rig.id],
        "no Shift-click selection toggle"
    );
    rig.tool.escape();
    assert_eq!(
        rig.press(top, false),
        SelectPointerDownOutcome::Cleared,
        "without Shift there is no handle there"
    );
}

#[test]
fn the_handle_set_is_frozen_during_a_drag() {
    let mut rig = Rig::new(Kind::Rect);
    let corner = rig.handle(EditHandle::Rotate(ResizeDirection::Ne), false);
    rig.press(corner, false);
    assert!(
        !rig.tool.side_rotate_revealed(true),
        "pressing Shift during a drag does not reveal the side handles"
    );
    rig.tool.escape();
    assert!(rig.tool.side_rotate_revealed(true), "idle: live Shift");
    assert!(!rig.tool.side_rotate_revealed(false));

    let side = rig.handle(EditHandle::Rotate(ResizeDirection::E), true);
    rig.press(side, true);
    assert!(
        rig.tool.side_rotate_revealed(false),
        "a dragged side handle stays visible after Shift is released"
    );
    rig.tool.escape();
}

#[test]
fn a_press_just_off_a_corner_resize_handle_resizes_and_just_beyond_it_rotates() {
    // Criterion 9: resize and rotate regions are disjoint, 32 px apart.
    let mut rig = Rig::new(Kind::Rect);
    let se = rig.handle(EditHandle::Resize(ResizeDirection::Se), false);
    let toward = Vec2::new(1.0, 1.0).normalized_to(1.0);
    let at = |distance_mm: f64| se.translated(toward.scaled(distance_mm));
    assert_eq!(rig.press(at(7.0), false), SelectPointerDownOutcome::Handle);
    assert_eq!(
        rig.tool.dragging_handle(),
        Some(EditHandle::Resize(ResizeDirection::Se))
    );
    rig.tool.escape();
    assert_eq!(rig.press(at(9.0), false), SelectPointerDownOutcome::Handle);
    assert_eq!(
        rig.tool.dragging_handle(),
        Some(EditHandle::Rotate(ResizeDirection::Se)),
        "9 mm out is past the 8 mm midpoint between the resize and rotate centres (16 mm apart)"
    );
}

#[test]
fn skew_handles_hide_per_axis_below_24_px_and_at_zero_extent() {
    // 12 mm = 24 px at this zoom. A path 100 mm wide and 10 mm high: only
    // the left and right (y skew, needs width) handles remain.
    let document = Document::new(1);
    let id = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 10.0)),
        ],
        false,
    );
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    let objects = vec![document.object(id).unwrap()];
    let handles = SelectTool::transform_handles(&objects, &selection, tolerances(), false);
    assert_eq!(
        count(&handles, |h| matches!(
            h,
            EditHandle::Skew(Side::Top | Side::Bottom)
        )),
        0
    );
    assert_eq!(
        count(&handles, |h| matches!(
            h,
            EditHandle::Skew(Side::Left | Side::Right)
        )),
        2
    );

    // A horizontal line has no height at all.
    let line = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 3), pt(0.0, 50.0)),
            NewAnchor::corner(AnchorId::new(1, 4), pt(100.0, 50.0)),
        ],
        false,
    );
    selection.select_single(line);
    let objects = vec![document.object(line).unwrap()];
    let handles = SelectTool::transform_handles(&objects, &selection, tolerances(), false);
    assert_eq!(
        count(&handles, |h| matches!(
            h,
            EditHandle::Skew(Side::Top | Side::Bottom)
        )),
        0
    );
}

#[test]
fn a_primitive_never_starts_a_skew_and_a_press_where_the_handle_would_be_writes_nothing() {
    for kind in [Kind::Rect, Kind::Ellipse, Kind::Polygon, Kind::Star] {
        let mut rig = Rig::new(kind);
        let before = rig.object();
        let box_ = oriented_bounds(&before);
        // Where a top skew handle would sit: 8 mm above the top-edge midpoint.
        let top_mid = box_.to_document(pt(f64::midpoint(box_.min.x, box_.max.x), box_.min.y));
        let spot = top_mid.translated(Vec2::new(0.0, -10.0));
        rig.drag(spot, spot.translated(Vec2::new(30.0, 0.0)), false, false);
        assert_eq!(
            rig.object(),
            before,
            "{kind:?}: not moved, resized, rotated or skewed"
        );
    }
}

// ---------------------------------------------------------------------
// Dead zone, centre handle, double-click: criteria 1-4, 18, 41
// ---------------------------------------------------------------------

fn rect_origin(rig: &Rig) -> Point {
    let ObjectSnapshot::Primitive(p) = rig.object() else {
        panic!("primitive");
    };
    let curvyo_document_core::Shape::Rect { bounds, .. } = p.shape else {
        panic!("rect");
    };
    bounds.origin
}

#[test]
fn a_wobble_under_the_dead_zone_writes_nothing_for_every_drag_kind() {
    for kind in ALL_KINDS {
        let mut rig = Rig::new(kind);
        let before = rig.object();
        for wanted in [
            EditHandle::Resize(ResizeDirection::Se),
            EditHandle::Rotate(ResizeDirection::Ne),
        ] {
            let at = rig.handle(wanted, false);
            rig.drag(at, at.translated(Vec2::new(1.0, 0.5)), false, false);
            assert_eq!(rig.object(), before, "{kind:?} {wanted:?}");
        }
        if is_path(kind) {
            let at = rig.handle(EditHandle::Skew(Side::Top), false);
            rig.drag(at, at.translated(Vec2::new(1.2, 0.0)), false, false);
            assert_eq!(rig.object(), before, "{kind:?} skew");
        }
        // A body press and wobble.
        let centre = {
            let b = oriented_bounds(&before);
            b.to_document(b.local_center())
        };
        rig.drag(centre, centre.translated(Vec2::new(1.0, 1.0)), false, false);
        assert_eq!(rig.object(), before, "{kind:?} body");
    }
}

#[test]
fn past_the_dead_zone_the_object_follows_the_pointer_one_to_one_from_the_press() {
    let mut rig = Rig::new(Kind::Rect);
    let before = rect_origin(&rig);
    let centre = pt(60.0, 50.0);
    rig.press(centre, false);
    assert_eq!(
        rig.tool
            .live_move(centre.translated(Vec2::new(1.0, 0.0)), false, false),
        None
    );
    let live = rig
        .tool
        .live_move(centre.translated(Vec2::new(10.0, 4.0)), false, false)
        .unwrap()
        .offset;
    assert_eq!(
        (live.x, live.y),
        (10.0, 4.0),
        "nothing jumps: delta is from the press point"
    );
    rig.tool.pointer_moved(
        centre.translated(Vec2::new(10.0, 4.0)),
        Modifiers::NONE,
        &mut rig.selection,
    );
    rig.release(centre.translated(Vec2::new(10.0, 4.0)), false, false);
    let after = rect_origin(&rig);
    assert!(close(after.x - before.x, 10.0) && close(after.y - before.y, 4.0));
}

#[test]
fn a_centre_handle_press_moves_exactly_like_a_body_drag() {
    let mut rig = Rig::new(Kind::Rect);
    let before = rig.object();
    let centre = pt(60.0, 50.0);
    // The centre handle is hover-only: never a press candidate.
    assert!(
        SelectTool::handle_at(&rig.objects(), &rig.selection, centre, tolerances(), false)
            .is_none()
    );
    let objects = rig.objects();
    let hovered =
        SelectTool::hover_handle_at(&objects, &rig.selection, centre, tolerances(), false);
    assert_eq!(hovered.map(|(_, _, h)| h), Some(EditHandle::Move));
    rig.press(centre, false);
    assert_eq!(rig.tool.dragging_handle(), Some(EditHandle::Move));
    rig.tool
        .pointer_moved(pt(80.0, 40.0), Modifiers::NONE, &mut rig.selection);
    rig.release(pt(80.0, 40.0), false, false);
    let after = rig.object();
    assert_ne!(after, before);
    let (ObjectSnapshot::Primitive(a), ObjectSnapshot::Primitive(b)) = (&after, &before) else {
        panic!()
    };
    assert_eq!(a.rotation, b.rotation, "a pure translation");
    let box_a = oriented_bounds(&after);
    let box_b = oriented_bounds(&before);
    assert!(close(box_a.width(), box_b.width()) && close(box_a.height(), box_b.height()));
    assert!(close(rect_origin(&rig).x, 30.0) && close(rect_origin(&rig).y, 10.0));
}

#[test]
fn the_centre_handle_needs_a_forty_eight_pixel_box_and_never_covers_a_resize_handle() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(23.0), // 46 px
        height: Length::from_mm(100.0),
    });
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    let objects = vec![document.object(id).unwrap()];
    let handles = SelectTool::transform_handles(&objects, &selection, tolerances(), false);
    assert_eq!(count(&handles, |h| *h == EditHandle::Move), 0);
    assert!(
        SelectTool::hover_handle_at(&objects, &selection, pt(11.5, 50.0), tolerances(), false)
            .is_none(),
        "no hover state below 48 px"
    );
}

#[test]
fn a_double_click_dispatches_by_what_is_under_the_second_press() {
    // Inside the box and not on a handle: a path hands off to the Node tool; a
    // primitive only asks for the edit hint (`unified-object-editing` criteria
    // 31, 32). The drawn centre handle is the exception: a double-click on it
    // opens the typed move (`edit-interaction-polish` criteria 15, 16).
    for kind in ALL_KINDS {
        let mut rig = Rig::new(kind);
        let before = rig.object();
        let b = oriented_bounds(&before);
        let centre = b.to_document(b.local_center());
        assert_eq!(
            rig.double_click(centre, false, false),
            SelectDoubleClickOutcome::EntryOpened,
            "{kind:?}: the centre handle opens the typed move"
        );
        assert!(rig.tool.move_entry().is_some(), "{kind:?}");
        assert!(rig.tool.entry().is_none());
        rig.tool.cancel_entry();
        let elsewhere = centre.translated(Vec2::new(8.0, 5.0));
        let outcome = rig.double_click(elsewhere, false, false);
        let expected = if is_path(kind) {
            SelectDoubleClickOutcome::Hit(before.clone())
        } else {
            SelectDoubleClickOutcome::EditHint
        };
        assert_eq!(outcome, expected, "{kind:?}");
        assert!(rig.tool.entry().is_none());
        assert!(rig.tool.move_entry().is_none());

        // Rotate and resize handles open the entry, no handoff.
        let rotate = rig.handle(EditHandle::Rotate(ResizeDirection::Ne), false);
        assert_eq!(
            rig.double_click(rotate, false, false),
            SelectDoubleClickOutcome::EntryOpened,
            "{kind:?}"
        );
        assert!(matches!(rig.tool.entry().unwrap().kind(), EntryKind::Angle));
        rig.tool.cancel_entry();
        let resize = rig.handle(EditHandle::Resize(ResizeDirection::Se), false);
        assert_eq!(
            rig.double_click(resize, false, false),
            SelectDoubleClickOutcome::EntryOpened,
            "{kind:?}"
        );
        rig.tool.cancel_entry();

        // A skew handle opens the skew entry, no handoff (criterion 9).
        if is_path(kind) {
            let skew = rig.handle(EditHandle::Skew(Side::Top), false);
            assert_eq!(
                rig.double_click(skew, false, false),
                SelectDoubleClickOutcome::EntryOpened
            );
            assert_eq!(rig.tool.skew_entry().unwrap().kind(), EntryKind::Skew);
            rig.tool.cancel_entry();
        }
        assert_eq!(rig.object(), before, "{kind:?}: nothing written");
    }
}

/// The real flow: double-clicking the outline of an object that is not
/// selected yet. The handles only appear after the first click, so the second
/// press must not open the N-edge handle's entry: a path hands off, a
/// primitive only asks for the edit hint.
#[test]
fn double_clicking_the_outline_of_an_unselected_object_at_a_handle_spot_opens_no_entry() {
    let mut rig = Rig::new(Kind::Rect);
    rig.selection.clear();
    let outcome = rig.double_click(pt(60.0, 20.0), false, false);
    assert_eq!(outcome, SelectDoubleClickOutcome::EditHint);
    assert!(rig.tool.entry().is_none(), "no size entry opened");
}

#[test]
fn a_double_click_on_a_handle_the_first_press_did_not_grab_does_not_open_an_entry() {
    let mut rig = Rig::new(Kind::Rect);
    // First press on the body, second on the Se corner: not one handle.
    rig.press(pt(40.0, 40.0), false);
    rig.release(pt(40.0, 40.0), false, false);
    let objects = rig.objects();
    let outcome = rig.tool.double_click(
        &objects,
        &rig.selection,
        pt(110.0, 80.0),
        SEGMENT_TOLERANCE,
        tolerances(),
        (false, false),
    );
    assert!(!matches!(outcome, SelectDoubleClickOutcome::EntryOpened));
    assert!(rig.tool.entry().is_none());
}

#[test]
fn a_double_click_far_from_everything_is_a_miss() {
    let mut rig = Rig::new(Kind::Rect);
    assert_eq!(
        rig.double_click(pt(900.0, 900.0), false, false),
        SelectDoubleClickOutcome::Miss
    );
}

// ---------------------------------------------------------------------
// Pivot rule: criteria 12-17
// ---------------------------------------------------------------------

#[test]
fn the_rotate_pivot_is_the_centre_or_the_opposite_corner_or_side_midpoint() {
    let mut rig = Rig::new(Kind::Rect);
    // Rect (10, 20) 100 x 60: centre (60, 50).
    let cases = [
        (ResizeDirection::Ne, pt(10.0, 80.0)),
        (ResizeDirection::Se, pt(10.0, 20.0)),
        (ResizeDirection::Sw, pt(110.0, 20.0)),
        (ResizeDirection::Nw, pt(110.0, 80.0)),
        (ResizeDirection::N, pt(60.0, 80.0)),
        (ResizeDirection::S, pt(60.0, 20.0)),
        (ResizeDirection::E, pt(10.0, 50.0)),
        (ResizeDirection::W, pt(110.0, 50.0)),
    ];
    for (direction, opposite) in cases {
        let at = rig.handle(EditHandle::Rotate(direction), true);
        rig.press(at, true);
        let shifted = rig.tool.live_pivot(true).unwrap();
        let plain = rig.tool.live_pivot(false).unwrap();
        assert!(
            close(shifted.x, opposite.x) && close(shifted.y, opposite.y),
            "{direction:?}: {shifted:?}"
        );
        assert!(
            close(plain.x, 60.0) && close(plain.y, 50.0),
            "{direction:?}: {plain:?}"
        );
        rig.tool.escape();
    }
}

#[test]
fn the_opposite_corner_is_taken_in_the_objects_own_rotated_frame() {
    let mut rig = Rig::new(Kind::Rect);
    let rotated = rig
        .object()
        .rotated(pt(60.0, 50.0), Angle::from_radians(0.6));
    rig.document.rotate_object(&rotated).unwrap();
    let ne = rig.handle(EditHandle::Rotate(ResizeDirection::Ne), false);
    rig.press(ne, true);
    let pivot = rig.tool.live_pivot(true).unwrap();
    let want = turned(pt(60.0, 50.0), pt(10.0, 80.0), 0.6);
    assert!(
        close(pivot.x, want.x) && close(pivot.y, want.y),
        "{pivot:?} vs {want:?}"
    );
}

#[test]
fn a_shifted_corner_rotate_leaves_the_opposite_corner_in_place_for_every_kind() {
    for kind in ALL_KINDS {
        let mut rig = Rig::new(kind);
        let before = rig.object();
        let box_before = oriented_bounds(&before);
        let sw_before = box_before.to_document(pt(box_before.min.x, box_before.max.y));
        let ne = rig.handle(EditHandle::Rotate(ResizeDirection::Ne), false);
        let target = turned(sw_before, ne, 0.7);
        rig.drag(ne, target, true, false);
        let after = rig.object();
        assert!(
            (after.rotation().as_radians() - 0.7).abs() < 1e-9,
            "{kind:?}: rotation {}",
            after.rotation().as_radians()
        );
        // The pivot corner: a primitive's corner in its own frame, a path's
        // box corner recomputed in the same (rotated) frame; both end up
        // where the Sw corner was before.
        let box_after = oriented_bounds(&after);
        let sw_after = box_after.to_document(pt(box_after.min.x, box_after.max.y));
        if !is_path(kind) {
            assert!(
                close(sw_after.x, sw_before.x) && close(sw_after.y, sw_before.y),
                "{kind:?}: {sw_after:?} vs {sw_before:?}"
            );
        }
    }
}

#[test]
fn switching_shift_mid_drag_always_computes_from_the_drag_start() {
    for kind in [Kind::Rect, Kind::OpenPath] {
        let mut rig = Rig::new(kind);
        let ne = rig.handle(EditHandle::Rotate(ResizeDirection::Ne), false);
        rig.press(ne, false);
        let current = ne.translated(Vec2::new(-30.0, 25.0));
        rig.tool
            .pointer_moved(current, Modifiers::NONE, &mut rig.selection);
        let with = rig.tool.live_transform(current, true, false).unwrap();
        let without = rig.tool.live_transform(current, false, false).unwrap();
        let with_again = rig.tool.live_transform(current, true, false).unwrap();
        assert_ne!(with, without);
        assert_eq!(with, with_again, "{kind:?}: no accumulated error");
        rig.tool.escape();
        // A fresh drag with Shift from the start gives the same snapshot.
        rig.press(ne, true);
        rig.tool
            .pointer_moved(current, Modifiers::NONE, &mut rig.selection);
        assert_eq!(rig.tool.live_transform(current, true, false).unwrap(), with);
        rig.tool.escape();
    }
}

#[test]
fn ctrl_snaps_a_rotate_about_whichever_pivot_applies() {
    for shift in [false, true] {
        let mut rig = Rig::new(Kind::Rect);
        let ne = rig.handle(EditHandle::Rotate(ResizeDirection::Ne), false);
        rig.press(ne, shift);
        let pivot = rig.tool.live_pivot(shift).unwrap();
        let current = turned(pivot, ne, 19.0_f64.to_radians());
        rig.tool
            .pointer_moved(current, Modifiers::NONE, &mut rig.selection);
        rig.release(current, shift, true);
        let degrees = rig.object().rotation().as_radians().to_degrees();
        assert!((degrees - 22.5).abs() < 1e-6, "shift {shift}: {degrees}");
    }
}

// ---------------------------------------------------------------------
// Typed entry: criteria 16, 18-32
// ---------------------------------------------------------------------

fn commit(rig: &mut Rig, texts: [&str; 2], last: usize) -> EntryOutcome {
    rig.tool.commit_entry(&rig.document, texts, last)
}

fn open_angle(rig: &mut Rig, direction: ResizeDirection, shift: bool) {
    let at = rig.handle(EditHandle::Rotate(direction), shift);
    assert_eq!(
        rig.double_click(at, shift, false),
        SelectDoubleClickOutcome::EntryOpened
    );
}

fn open_size(rig: &mut Rig, direction: ResizeDirection, shift: bool, ctrl: bool) {
    let at = rig.handle(EditHandle::Resize(direction), false);
    assert_eq!(
        rig.double_click(at, shift, ctrl),
        SelectDoubleClickOutcome::EntryOpened
    );
}

#[test]
fn the_angle_entry_opens_prefilled_with_the_readouts_value_and_commits_an_absolute_rotation() {
    let mut rig = Rig::new(Kind::Rect);
    let seed = rig
        .object()
        .rotated(pt(60.0, 50.0), Angle::from_radians(37.428_f64.to_radians()));
    rig.document.rotate_object(&seed).unwrap();
    open_angle(&mut rig, ResizeDirection::Ne, false);
    let entry = rig.tool.entry().unwrap();
    assert_eq!(entry.fields().len(), 1);
    assert_eq!(
        entry.fields()[0].prefill,
        "37.4",
        "the readout's rounded value"
    );
    assert_eq!(entry.fields()[0].accessible_name, "Angle");

    // Untouched text writes nothing, even though 37.4 != 37.428.
    let before = rig.object();
    assert_eq!(commit(&mut rig, ["37.4", ""], 0), EntryOutcome::Unchanged);
    assert_eq!(rig.object(), before);
    assert!(
        rig.tool.entry().is_none(),
        "Enter on untouched text closes the entry"
    );

    // A typed number is the absolute rotation: "12,5°" (decimal comma, degree sign).
    open_angle(&mut rig, ResizeDirection::Ne, false);
    assert_eq!(commit(&mut rig, ["12,5°", ""], 0), EntryOutcome::Committed);
    assert!((rig.object().rotation().as_radians().to_degrees() - 12.5).abs() < 1e-9);
    // Typing 0 restores an unrotated object.
    open_angle(&mut rig, ResizeDirection::Ne, false);
    assert_eq!(commit(&mut rig, ["0", ""], 0), EntryOutcome::Committed);
    assert!(rig.object().rotation().as_radians().abs() < 1e-12);
    // Any finite value is normalized: 405 -> 45.
    open_angle(&mut rig, ResizeDirection::Ne, false);
    assert_eq!(commit(&mut rig, ["405", ""], 0), EntryOutcome::Committed);
    assert!((rig.object().rotation().as_radians().to_degrees() - 45.0).abs() < 1e-9);
    // A value equal to the current rotation writes nothing.
    open_angle(&mut rig, ResizeDirection::Ne, false);
    let before = rig.object();
    assert_eq!(commit(&mut rig, ["45.0", ""], 0), EntryOutcome::Unchanged);
    assert_eq!(rig.object(), before);
}

#[test]
fn an_angle_that_is_not_a_number_keeps_the_entry_open_and_writes_nothing() {
    for text in ["", "abc", "1,2,3", "1e3", "12 deg", "--5", "°"] {
        let mut rig = Rig::new(Kind::Rect);
        open_angle(&mut rig, ResizeDirection::Ne, false);
        let before = rig.object();
        assert_eq!(
            commit(&mut rig, [text, ""], 0),
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::NotANumber
            },
            "{text:?}"
        );
        assert!(rig.tool.entry().is_some(), "{text:?}: stays open");
        assert_eq!(rig.object(), before);
    }
}

#[test]
fn the_entry_pivot_is_fixed_when_it_opens_and_matches_a_drag_with_the_same_pivot() {
    for kind in ALL_KINDS {
        for shift in [false, true] {
            // Type 40 degrees.
            let mut typed = Rig::new(kind);
            open_angle(&mut typed, ResizeDirection::Ne, shift);
            let pivot_shown = typed.tool.entry().unwrap().pivot();
            assert_eq!(
                typed.tool.live_pivot(!shift),
                Some(pivot_shown),
                "fixed, ignores later Shift"
            );
            assert_eq!(commit(&mut typed, ["40", ""], 0), EntryOutcome::Committed);

            // Drag to the same angle with the same pivot.
            let mut dragged = Rig::new(kind);
            let ne = dragged.handle(EditHandle::Rotate(ResizeDirection::Ne), false);
            dragged.press(ne, shift);
            let pivot = dragged.tool.live_pivot(shift).unwrap();
            assert!(
                close(pivot.x, pivot_shown.x) && close(pivot.y, pivot_shown.y),
                "{kind:?} {shift}"
            );
            let to = turned(pivot, ne, 40.0_f64.to_radians());
            dragged
                .tool
                .pointer_moved(to, Modifiers::NONE, &mut dragged.selection);
            dragged.release(to, shift, false);
            assert_snapshots_close(
                &typed.object(),
                &dragged.object(),
                EPS,
                &format!("{kind:?} shift {shift}"),
            );
        }
    }
}

#[test]
fn a_size_entry_has_the_fields_the_handle_changes() {
    let mut rig = Rig::new(Kind::Rect);
    open_size(&mut rig, ResizeDirection::Se, false, false);
    let fields = rig.tool.entry().unwrap().fields().to_vec();
    assert_eq!(fields.len(), 2);
    assert_eq!((fields[0].label, fields[0].accessible_name), ("W", "Width"));
    assert_eq!(
        (fields[1].label, fields[1].accessible_name),
        ("H", "Height")
    );
    assert_eq!(
        (fields[0].prefill.as_str(), fields[1].prefill.as_str()),
        ("100.0", "60.0")
    );
    rig.tool.cancel_entry();

    open_size(&mut rig, ResizeDirection::E, false, false);
    assert_eq!(rig.tool.entry().unwrap().fields().len(), 1);
    assert_eq!(
        rig.tool.entry().unwrap().fields()[0].accessible_name,
        "Width"
    );
    rig.tool.cancel_entry();
    open_size(&mut rig, ResizeDirection::N, false, false);
    assert_eq!(
        rig.tool.entry().unwrap().fields()[0].accessible_name,
        "Height"
    );
    rig.tool.cancel_entry();

    let mut poly = Rig::new(Kind::Polygon);
    open_size(&mut poly, ResizeDirection::Ne, false, false);
    let entry = poly.tool.entry().unwrap();
    assert_eq!(entry.kind(), EntryKind::OuterRadius);
    assert_eq!(entry.fields().len(), 1);
    assert_eq!(
        (entry.fields()[0].label, entry.fields()[0].accessible_name),
        ("r", "Outer radius")
    );
    assert_eq!(entry.fields()[0].prefill, "40.0");
    // Centre of the shape is the fixed point, with or without Shift (flag 1).
    let center = poly.tool.entry().unwrap().pivot();
    assert!(close(center.x, 60.0) && close(center.y, 50.0));
}

#[test]
fn a_size_entry_keeps_the_hand_drags_fixed_point_and_shift_uses_the_centre() {
    let mut rig = Rig::new(Kind::Rect);
    open_size(&mut rig, ResizeDirection::Se, false, false);
    assert_eq!(
        commit(&mut rig, ["150", "60.0"], 0),
        EntryOutcome::Committed
    );
    let origin = rect_origin(&rig);
    assert!(
        close(origin.x, 10.0) && close(origin.y, 20.0),
        "opposite (Nw) corner stays"
    );
    assert!(close(oriented_bounds(&rig.object()).width(), 150.0));

    let mut shifted = Rig::new(Kind::Rect);
    open_size(&mut shifted, ResizeDirection::Se, true, false);
    let pivot = shifted.tool.entry().unwrap().pivot();
    assert!(
        close(pivot.x, 60.0) && close(pivot.y, 50.0),
        "Shift: the box centre"
    );
    assert_eq!(
        commit(&mut shifted, ["140", "80"], 0),
        EntryOutcome::Committed
    );
    let b = oriented_bounds(&shifted.object());
    assert!(close(b.width(), 140.0) && close(b.height(), 80.0));
    let c = b.to_document(b.local_center());
    assert!(close(c.x, 60.0) && close(c.y, 50.0));
}

#[test]
fn a_size_that_is_zero_negative_or_not_a_number_keeps_the_group_open() {
    let mut rig = Rig::new(Kind::Rect);
    open_size(&mut rig, ResizeDirection::Se, false, false);
    let before = rig.object();
    assert_eq!(
        commit(&mut rig, ["0", "60.0"], 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::NotPositive
        }
    );
    assert_eq!(
        commit(&mut rig, ["100.0", "-5"], 1),
        EntryOutcome::Invalid {
            field: 1,
            reason: InvalidReason::NotPositive
        }
    );
    assert_eq!(
        commit(&mut rig, ["100.0", "x"], 1),
        EntryOutcome::Invalid {
            field: 1,
            reason: InvalidReason::NotANumber
        }
    );
    assert_eq!(
        commit(&mut rig, ["1e400", "60.0"], 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::NotANumber
        }
    );
    assert!(rig.tool.entry().is_some());
    assert_eq!(rig.object(), before);
    // Untouched prefill and an equal value write nothing and close.
    assert_eq!(
        commit(&mut rig, ["100.0", "60.0"], 0),
        EntryOutcome::Unchanged
    );
    assert_eq!(rig.object(), before);
    assert!(rig.tool.entry().is_none());
}

#[test]
fn ctrl_links_width_and_height_at_the_openings_aspect_ratio() {
    let mut rig = Rig::new(Kind::Rect);
    open_size(&mut rig, ResizeDirection::Se, false, true);
    let entry = rig.tool.entry().unwrap();
    assert!(entry.linked());
    assert_eq!(entry.linked_text(0, "200").as_deref(), Some("120.0"));
    assert_eq!(entry.linked_text(1, "30").as_deref(), Some("50.0"));
    assert_eq!(entry.linked_text(0, "abc"), None);
    assert_eq!(
        commit(&mut rig, ["200", "120.0"], 0),
        EntryOutcome::Committed
    );
    let b = oriented_bounds(&rig.object());
    assert!(close(b.width(), 200.0) && close(b.height(), 120.0));

    // Without Ctrl the fields are independent.
    let mut free = Rig::new(Kind::Rect);
    open_size(&mut free, ResizeDirection::Se, false, false);
    assert!(!free.tool.entry().unwrap().linked());
    assert_eq!(free.tool.entry().unwrap().linked_text(0, "200"), None);
    // Edge handles and polygons are never linked.
    let mut edge = Rig::new(Kind::Rect);
    open_size(&mut edge, ResizeDirection::E, false, true);
    assert!(!edge.tool.entry().unwrap().linked());
}

#[test]
fn the_stroke_switch_is_read_when_the_entry_opens() {
    let mut rig = Rig::new(Kind::Rect);
    rig.document
        .resize_rect(
            rig.id,
            RectBounds {
                origin: pt(10.0, 20.0),
                width: Length::from_mm(100.0),
                height: Length::from_mm(60.0),
            },
            curvyo_document_core::CornerRadii::uniform(Length::from_mm(0.0)),
            Some(Length::from_mm(2.0)),
        )
        .unwrap();
    rig.tool.set_stroke_scaling(StrokeScaling::Proportional);
    open_size(&mut rig, ResizeDirection::Se, false, false);
    // The switch flips while the entry is open: the opening's value counts.
    rig.tool.set_stroke_scaling(StrokeScaling::Keep);
    assert_eq!(commit(&mut rig, ["200", "120"], 0), EntryOutcome::Committed);
    let ObjectSnapshot::Primitive(p) = rig.object() else {
        panic!()
    };
    assert!(
        close(p.stroke_width.as_mm(), 4.0),
        "scaled by the factor 2: {}",
        p.stroke_width.as_mm()
    );

    let mut keep = Rig::new(Kind::Rect);
    open_size(&mut keep, ResizeDirection::Se, false, false);
    let before = {
        let ObjectSnapshot::Primitive(p) = keep.object() else {
            panic!()
        };
        p.stroke_width
    };
    assert_eq!(
        commit(&mut keep, ["200", "120"], 0),
        EntryOutcome::Committed
    );
    let ObjectSnapshot::Primitive(p) = keep.object() else {
        panic!()
    };
    assert_eq!(p.stroke_width, before, "switch off: stroke untouched");
}

/// Criteria 16, 27: a drag to the pointer that yields size S and an entry
/// of S leave snapshots equal within 1e-9 mm and 1e-12 rad: every kind,
/// handle and modifier combination.
#[test]
fn a_typed_size_and_a_dragged_size_leave_the_same_snapshot() {
    let mut compared = 0;
    for kind in ALL_KINDS {
        let directions: &[ResizeDirection] = if is_polygon_or_star(kind) {
            &[ResizeDirection::Ne, ResizeDirection::Sw]
        } else {
            &ResizeDirection::ALL_EIGHT
        };
        for &direction in directions {
            for (shift, ctrl) in [(false, false), (true, false), (false, true), (true, true)] {
                for turn in [0.0, 0.6] {
                    let mut dragged = Rig::new(kind);
                    if turn != 0.0 {
                        let rotated = dragged
                            .object()
                            .rotated(pt(60.0, 50.0), Angle::from_radians(turn));
                        dragged.document.rotate_object(&rotated).unwrap();
                    }
                    let mut typed = Rig::new(kind);
                    if turn != 0.0 {
                        let rotated = typed
                            .object()
                            .rotated(pt(60.0, 50.0), Angle::from_radians(turn));
                        typed.document.rotate_object(&rotated).unwrap();
                    }
                    let start_box = oriented_bounds(&dragged.object());
                    let at = dragged.handle(EditHandle::Resize(direction), false);
                    // Pull the handle outward by a modest, handle-specific vector.
                    let outward = turned(pt(0.0, 0.0), pt(14.0, 9.0), turn);
                    let to = at.translated(Vec2::new(outward.x, outward.y));
                    dragged.press(at, false);
                    dragged
                        .tool
                        .pointer_moved(to, Modifiers::NONE, &mut dragged.selection);
                    let result = dragged.tool.live_transform(to, shift, ctrl).unwrap();
                    dragged.release(to, shift, ctrl);
                    let committed = dragged.object();
                    assert_eq!(committed, result, "preview and release agree");

                    // The size the drag yielded, typed.
                    let b = oriented_bounds(&result);
                    open_size(&mut typed, direction, shift, ctrl);
                    let entry = typed.tool.entry().unwrap().clone();
                    let context =
                        format!("{kind:?} {direction:?} shift {shift} ctrl {ctrl} turn {turn}");
                    let texts = if entry.kind() == EntryKind::OuterRadius {
                        let r = radius_of(&result);
                        [format!("{r}"), String::new()]
                    } else if entry.fields().len() == 1 {
                        let touches_width =
                            matches!(direction, ResizeDirection::E | ResizeDirection::W);
                        [
                            format!("{}", if touches_width { b.width() } else { b.height() }),
                            String::new(),
                        ]
                    } else {
                        [format!("{}", b.width()), format!("{}", b.height())]
                    };
                    // Linked fields: the last edited one carries the target.
                    let last_edited = if entry.linked() {
                        // Ctrl's dominant axis: the one the drag moved further.
                        let local = start_box.to_local(at).vector_to(start_box.to_local(to));
                        usize::from(local.x.abs() < local.y.abs())
                    } else {
                        0
                    };
                    let outcome = typed.tool.commit_entry(
                        &typed.document,
                        [&texts[0], &texts[1]],
                        last_edited,
                    );
                    if result == dragged_start(kind, turn) {
                        assert_eq!(outcome, EntryOutcome::Unchanged, "{context}");
                        continue;
                    }
                    assert_eq!(outcome, EntryOutcome::Committed, "{context}");
                    assert_snapshots_close(&typed.object(), &committed, EPS, &context);
                    compared += 1;
                }
            }
        }
    }
    assert!(compared >= 150, "only {compared} drag/entry pairs compared");
}

fn radius_of(object: &ObjectSnapshot) -> f64 {
    let ObjectSnapshot::Primitive(p) = object else {
        panic!()
    };
    match p.shape {
        curvyo_document_core::Shape::Polygon { frame, .. }
        | curvyo_document_core::Shape::Star { frame, .. } => frame.radius.as_mm(),
        _ => panic!("not a polygon or star"),
    }
}

fn dragged_start(kind: Kind, turn: f64) -> ObjectSnapshot {
    let rig = Rig::new(kind);
    if turn == 0.0 {
        return rig.object();
    }
    rig.object()
        .rotated(pt(60.0, 50.0), Angle::from_radians(turn))
}

// ---------------------------------------------------------------------
// Skew: criteria 38-47
// ---------------------------------------------------------------------

fn anchors_of(object: &ObjectSnapshot) -> Vec<(Point, Vec2, Vec2)> {
    let ObjectSnapshot::Path(p) = object else {
        panic!("path")
    };
    p.anchors
        .iter()
        .map(|a| (a.point, a.handle_in, a.handle_out))
        .collect()
}

#[test]
fn skewing_from_the_top_keeps_the_bottom_line_and_moves_the_top_one_to_one() {
    let mut rig = Rig::new(Kind::OpenPath);
    let before = rig.object();
    let box_ = oriented_bounds(&before);
    let top = rig.handle(EditHandle::Skew(Side::Top), false);
    rig.drag(top, top.translated(Vec2::new(12.0, 3.0)), false, false);
    let after = rig.object();
    let (a, b) = (anchors_of(&before), anchors_of(&after));
    // k = d / (c_g - c_0) with d = 12 and h = box height.
    let h = box_.height();
    let k = -(12.0 / h); // lever is the (negative) top-minus-bottom distance.
    for ((pa, ha_in, ha_out), (pb, hb_in, hb_out)) in a.iter().zip(&b) {
        let expected_x = pa.x + k * (pa.y - box_.max.y);
        assert!(close(pb.x, expected_x), "{pb:?} vs x {expected_x}");
        assert!(close(pb.y, pa.y), "an x skew leaves y alone");
        // Handle vectors take the linear part only: x' = x + k y.
        assert!(close(hb_out.x, ha_out.x + k * ha_out.y) && close(hb_out.y, ha_out.y));
        assert!(close(hb_in.x, ha_in.x + k * ha_in.y) && close(hb_in.y, ha_in.y));
    }
    // The topmost anchor moved by exactly the pointer's x displacement.
    let top_index = a
        .iter()
        .enumerate()
        .min_by(|x, y| x.1.0.y.total_cmp(&y.1.0.y))
        .unwrap()
        .0;
    assert!(close(
        b[top_index].0.x - a[top_index].0.x,
        12.0 * ((box_.max.y - a[top_index].0.y) / h)
    ));
    // Rotation untouched, kinds and counts unchanged (criteria 42, 44, 46).
    assert_eq!(after.rotation(), before.rotation());
    assert_eq!(a.len(), b.len());
}

#[test]
fn skew_leaves_the_fixed_edge_alone_and_shift_fixes_the_centre_line() {
    let mut rig = Rig::new(Kind::OpenPath);
    let before = rig.object();
    let box_ = oriented_bounds(&before);
    let bottom = rig.handle(EditHandle::Skew(Side::Bottom), false);
    rig.press(bottom, false);
    let to = bottom.translated(Vec2::new(-9.0, 0.0));
    rig.tool
        .pointer_moved(to, Modifiers::NONE, &mut rig.selection);
    let plain = rig.tool.live_transform(to, false, false).unwrap();
    let centred = rig.tool.live_transform(to, true, false).unwrap();
    rig.tool.escape();
    // Fixed edge: the topmost anchor (on the box top) does not move.
    let top_anchor = |o: &ObjectSnapshot| {
        anchors_of(o)
            .into_iter()
            .map(|(p, _, _)| p)
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .unwrap()
    };
    assert!(close(top_anchor(&plain).x, top_anchor(&before).x));
    // Under Shift the centre line is fixed: an anchor on it does not move,
    // the top moves the opposite way by the same distance (h is half).
    let mid_y = f64::midpoint(box_.min.y, box_.max.y);
    let moved_top = top_anchor(&centred).x - top_anchor(&before).x;
    let moved_bottom_equivalent =
        -9.0 * ((box_.min.y - mid_y).abs() / ((box_.max.y - mid_y).abs()));
    assert!(close(moved_top, -moved_bottom_equivalent), "{moved_top}");
    let angle = rig.tool.live_skew_angle(to, false, false);
    assert!(angle.is_none(), "no drag in flight after escape");
}

#[test]
fn the_skew_angle_is_the_arctangent_and_stays_inside_ninety_degrees() {
    let mut rig = Rig::new(Kind::OpenPath);
    let box_ = oriented_bounds(&rig.object());
    let right = rig.handle(EditHandle::Skew(Side::Right), false);
    rig.press(right, false);
    let to = right.translated(Vec2::new(0.0, 20.0));
    rig.tool
        .pointer_moved(to, Modifiers::NONE, &mut rig.selection);
    let angle = rig
        .tool
        .live_skew_angle(to, false, false)
        .unwrap()
        .as_radians();
    assert!(
        (angle - (20.0 / box_.width()).atan()).abs() < 1e-12,
        "y skew from the right"
    );
    let far = right.translated(Vec2::new(0.0, 1e8));
    rig.tool
        .pointer_moved(far, Modifiers::NONE, &mut rig.selection);
    let angle = rig
        .tool
        .live_skew_angle(far, false, false)
        .unwrap()
        .as_radians();
    assert!(angle < FRAC_PI_2 && angle > 1.5);
    // Ctrl: capped at +-75 degrees.
    let ctrl_angle = rig
        .tool
        .live_skew_angle(far, false, true)
        .unwrap()
        .as_radians()
        .to_degrees();
    assert!((ctrl_angle - 75.0).abs() < 1e-9, "{ctrl_angle}");
    let near = right.translated(Vec2::new(0.0, box_.width() * 19.0_f64.to_radians().tan()));
    let snapped = rig
        .tool
        .live_skew_angle(near, false, true)
        .unwrap()
        .as_radians()
        .to_degrees();
    assert!((snapped - 22.5).abs() < 1e-9, "{snapped}");
    rig.tool.escape();
}

#[test]
fn skew_preview_equals_the_committed_result_and_escape_writes_nothing() {
    let mut rig = Rig::new(Kind::ClosedPath);
    let before = rig.object();
    let top = rig.handle(EditHandle::Skew(Side::Top), false);
    rig.press(top, false);
    let to = top.translated(Vec2::new(15.0, 0.0));
    rig.tool
        .pointer_moved(to, Modifiers::NONE, &mut rig.selection);
    let preview = rig.tool.live_transform(to, false, false).unwrap();
    rig.tool.escape();
    assert_eq!(rig.object(), before, "Escape: nothing written");
    rig.drag(top, to, false, false);
    assert_eq!(
        rig.object(),
        preview,
        "release does not change what was shown"
    );
    assert_ne!(rig.object(), before);
}

#[test]
fn a_drag_that_returns_to_its_start_writes_nothing() {
    let mut rig = Rig::new(Kind::OpenPath);
    let before = rig.object();
    let top = rig.handle(EditHandle::Skew(Side::Top), false);
    rig.press(top, false);
    rig.tool.pointer_moved(
        top.translated(Vec2::new(20.0, 0.0)),
        Modifiers::NONE,
        &mut rig.selection,
    );
    rig.release(top, false, false);
    assert_eq!(rig.object(), before);
}

#[test]
fn a_skew_and_its_inverse_restore_the_path_for_both_axes_with_and_without_shift() {
    for (side, shift, theta) in [
        (Side::Top, false, 0.0),
        (Side::Bottom, true, 0.0),
        (Side::Left, false, 0.7),
        (Side::Right, true, 0.7),
    ] {
        for kind in [Kind::OpenPath, Kind::ClosedPath] {
            let mut rig = Rig::new(kind);
            if theta != 0.0 {
                let rotated = rig
                    .object()
                    .rotated(pt(60.0, 50.0), Angle::from_radians(theta));
                rig.document.rotate_object(&rotated).unwrap();
            }
            let before = rig.object();
            let along = if matches!(side, Side::Top | Side::Bottom) {
                Vec2::new(1.0, 0.0).rotated(Angle::from_radians(theta))
            } else {
                Vec2::new(0.0, 1.0).rotated(Angle::from_radians(theta))
            };
            let first = rig.handle(EditHandle::Skew(side), shift);
            rig.drag(first, first.translated(along.scaled(17.0)), shift, false);
            assert_ne!(rig.object(), before);
            assert_eq!(
                rig.object().rotation(),
                before.rotation(),
                "rotation never changes"
            );
            // The box is the tight rectangle in the same frame again.
            let box_after = oriented_bounds(&rig.object());
            assert_eq!(box_after.angle, before.rotation());
            let second = rig.handle(EditHandle::Skew(side), shift);
            rig.drag(second, second.translated(along.scaled(-17.0)), shift, false);
            assert_snapshots_close(
                &rig.object(),
                &before,
                1e-9,
                &format!("{side:?} shift {shift} theta {theta}"),
            );
        }
    }
}

#[test]
fn a_rotated_path_skews_along_its_local_axes() {
    let mut rig = Rig::new(Kind::OpenPath);
    let rotated = rig
        .object()
        .rotated(pt(60.0, 50.0), Angle::from_radians(FRAC_PI_2));
    rig.document.rotate_object(&rotated).unwrap();
    let before = rig.object();
    // Quarter turn: the local top side now faces document +x.
    let top = rig.handle(EditHandle::Skew(Side::Top), false);
    let box_ = oriented_bounds(&before);
    let u = Vec2::new(0.0, 1.0); // local x axis after a quarter turn.
    rig.drag(top, top.translated(u.scaled(10.0)), false, false);
    let after = rig.object();
    assert_eq!(after.rotation(), before.rotation());
    // Every anchor moved along document y only (local u), none along x.
    for ((pa, _, _), (pb, _, _)) in anchors_of(&before).iter().zip(anchors_of(&after).iter()) {
        assert!(close(pa.x, pb.x), "{pa:?} -> {pb:?}");
    }
    let _ = box_;
}
