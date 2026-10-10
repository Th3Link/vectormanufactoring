//! `curvyo-ui-core` tests of the stretch that converts shapes to paths
//! (`specs/0019-multi-object-transform/` criteria 18 to 20, 32, 34, 53 to 56 and
//! `adrs.md` decision 8): which gestures convert, what the converted path is, that
//! the preview is the commit, and the counts the notice and the hint lines use.
//! Scale is 4 px/mm.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    missing_docs
)]
#![allow(clippy::too_many_lines)]

use curvyo_document_core::{
    Angle, Document, EllipseFrame, InnerRatio, Length, NodeId, ObjectSnapshot, Point, PointCount,
    PrimitiveSnapshot, RectBounds, Shape, StarFrame, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, EditHandle, EntryKey, EntryOutcome, Modifiers, ObjectSelection, PressTarget,
    ResizeDirection, SelectTool, TransformHandleTolerances, classify_press,
};

const SCALE: f64 = 4.0;
const NONE: Modifiers = Modifiers::NONE;
const CTRL: Modifiers = Modifiers::new(false, true);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(SCALE)
}

fn tol() -> Tolerance {
    Tolerance::from_mm(2.0)
}

struct Rig {
    document: Document,
    selection: ObjectSelection,
    tool: SelectTool,
}

impl Rig {
    fn new() -> Self {
        Self {
            document: Document::new(1),
            selection: ObjectSelection::new(),
            tool: SelectTool::new(),
        }
    }

    fn rect(&self, x: f64, y: f64, w: f64, h: f64) -> NodeId {
        self.document.create_rect(RectBounds {
            origin: pt(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        })
    }

    /// A 5-point star with its leftmost point at `left`.
    fn star(&self, left: f64, radius: f64) -> NodeId {
        self.document.create_star(
            StarFrame {
                center: pt(left + radius, 30.0),
                radius: Length::from_mm(radius),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        )
    }

    fn turned(&self, id: NodeId, degrees: f64) {
        let object = self.document.object(id).unwrap();
        let centre = pt(
            f64::midpoint(object_bounds(&object).0.x, object_bounds(&object).1.x),
            f64::midpoint(object_bounds(&object).0.y, object_bounds(&object).1.y),
        );
        let rotated = object.rotated(centre, Angle::from_radians(degrees.to_radians()));
        self.document.rotate_object(&rotated).unwrap();
    }

    fn objects(&self) -> Vec<ObjectSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.object(id))
            .collect()
    }

    fn select(&mut self, ids: &[NodeId]) {
        self.selection.set(ids);
    }

    fn group_box(&self) -> (Point, Point) {
        let group = SelectTool::group_of(&self.objects(), &self.selection).unwrap();
        (group.bounds().min, group.bounds().max)
    }

    fn east_edge(&self) -> Point {
        let (min, max) = self.group_box();
        pt(max.x, f64::midpoint(min.y, max.y))
    }

    fn south_east_corner(&self) -> Point {
        self.group_box().1
    }

    fn press(&mut self, at: Point, modifiers: Modifiers) {
        let objects = self.objects();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            at,
            tol(),
            tolerances(),
            modifiers,
        );
        self.tool.mint_conversion_ids(&mut AnchorIdMinter::new(9));
    }

    fn release(&mut self, to: Point, modifiers: Modifiers) {
        let objects = self.objects();
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            to,
            modifiers,
            &mut AnchorIdMinter::new(9),
        );
    }

    fn drag(&mut self, from: Point, to: Point, modifiers: Modifiers) {
        self.press(from, modifiers);
        self.tool.pointer_moved(
            pt(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y)),
            modifiers,
            &mut self.selection,
        );
        self.tool.pointer_moved(to, modifiers, &mut self.selection);
        self.release(to, modifiers);
    }

    fn live(&self, to: Point, modifiers: Modifiers) -> Option<Vec<ObjectSnapshot>> {
        self.tool
            .live_edit(
                &self.objects(),
                &self.selection,
                to,
                modifiers.shift,
                modifiers.ctrl,
            )
            .map(|live| live.objects)
    }

    fn target(&self, at: Point) -> PressTarget {
        classify_press(
            &self.objects(),
            &self.selection,
            at,
            tol(),
            tolerances(),
            NONE,
        )
    }
}

fn object_bounds(object: &ObjectSnapshot) -> (Point, Point) {
    let group = SelectTool::group_of(std::slice::from_ref(object), &{
        let mut s = ObjectSelection::new();
        s.set(&[object.id()]);
        s
    });
    // A single object has no group: fall back to its drawn bounds.
    group.map_or_else(
        || curvyo_ui_core::object_outline_bounds(object),
        |g| (g.bounds().min, g.bounds().max),
    )
}

fn is_path(document: &Document, id: NodeId) -> bool {
    document.path(id).is_some()
}

/// The criterion 20 example: a 5-point star at the left (its leftmost point at
/// x = 0) and an unrotated rectangle from (35, 0) to (45, 10): the group box is
/// x = 0 to 45; the right edge handle dragged to x = 90 gives sx = 2, sy = 1.
fn star_and_rect() -> (Rig, NodeId, NodeId) {
    let rig = Rig::new();
    let star = rig.star(0.0, 10.0);
    let rect = rig.rect(35.0, 20.0, 10.0, 10.0);
    (rig, star, rect)
}

/// Criteria 20 and 53: an edge drag stretches the rectangle as a rectangle and turns
/// the star into a closed path of ten nodes, in place, with its style.
#[test]
fn an_edge_stretch_converts_the_star_and_keeps_the_rectangle() {
    let (mut rig, star, rect) = star_and_rect();
    rig.select(&[star, rect]);
    let before = rig.document.object(star).unwrap();
    let ids_before = rig.document.object_ids();
    let east = rig.east_edge();
    assert_eq!(
        rig.target(east),
        PressTarget::Handle(EditHandle::Resize(ResizeDirection::E))
    );
    let (min, max) = rig.group_box();
    rig.drag(east, pt(min.x + (max.x - min.x) * 2.0, east.y), NONE);

    let ObjectSnapshot::Path(path) = rig.document.object(star).unwrap() else {
        panic!("the star is a path now");
    };
    assert_eq!(path.anchors.len(), 10);
    assert!(path.closed);
    assert_eq!(&path.style, before.style(), "same style");
    assert_eq!(rig.document.object_ids(), ids_before, "same place");
    // Twice its distance from the left edge, the same y.
    let ObjectSnapshot::Primitive(PrimitiveSnapshot {
        shape: Shape::Rect { bounds, .. },
        ..
    }) = rig.document.object(rect).unwrap()
    else {
        panic!("still a rectangle");
    };
    assert!((bounds.width.as_mm() - 20.0).abs() < 1e-6);
    assert!((bounds.height.as_mm() - 10.0).abs() < 1e-6);
    let (low, high) = curvyo_ui_core::object_outline_bounds(&rig.document.object(star).unwrap());
    let (b_low, b_high) = curvyo_ui_core::object_outline_bounds(&before);
    // About the left side of the group box (the star's leftmost point).
    assert!(
        (low.x - b_low.x).abs() < 1e-6
            && (high.x - (b_low.x + 2.0 * (b_high.x - b_low.x))).abs() < 1e-6
    );
    assert!((low.y - b_low.y).abs() < 1e-6 && (high.y - b_high.y).abs() < 1e-6);
    // The notice counts it once.
    assert_eq!(rig.tool.take_conversion_notice().as_array(), [0, 1, 0, 0]);
    assert!(rig.tool.take_conversion_notice().is_empty(), "taken once");
}

/// Criterion 19 (lead decision E): a corner drag of a selection that holds a star is
/// proportional whatever Ctrl does, and converts nothing.
#[test]
fn a_corner_drag_with_a_star_selected_stays_proportional() {
    for modifiers in [NONE, CTRL] {
        let (mut rig, star, rect) = star_and_rect();
        rig.select(&[star, rect]);
        let corner = rig.south_east_corner();
        // A very uneven drag: the dominant axis decides.
        rig.drag(corner, pt(corner.x + 40.0, corner.y + 2.0), modifiers);
        assert!(
            !is_path(&rig.document, star),
            "{modifiers:?}: the star stays a star"
        );
        assert!(rig.document.primitive(rect).is_some());
        let Shape::Rect { bounds, .. } = (match rig.document.object(rect).unwrap() {
            ObjectSnapshot::Primitive(p) => p.shape,
            ObjectSnapshot::Path(_) => panic!(),
        }) else {
            panic!()
        };
        assert!(
            (bounds.width.as_mm() - bounds.height.as_mm()).abs() < 1e-6,
            "one factor for both axes"
        );
        assert!(bounds.width.as_mm() > 10.0);
        assert!(rig.tool.take_conversion_notice().is_empty(), "no notice");
    }
}

/// Criterion 18: a selection with no converting shape keeps the free corner stretch
/// (and no notice).
#[test]
fn a_selection_without_converting_shapes_stretches_freely_at_a_corner() {
    let mut rig = Rig::new();
    let a = rig.rect(0.0, 0.0, 10.0, 10.0);
    let b = rig.rect(20.0, 0.0, 10.0, 10.0);
    rig.select(&[a, b]);
    let corner = rig.south_east_corner();
    rig.drag(corner, pt(corner.x + 30.0, corner.y + 20.0), NONE);
    let (min, max) = rig.group_box();
    assert!((max.x - min.x - 60.0).abs() < 1e-6 && (max.y - min.y - 30.0).abs() < 1e-6);
    assert!(rig.document.primitive(a).is_some() && rig.document.primitive(b).is_some());
    assert!(rig.tool.take_conversion_notice().is_empty());
}

/// Criterion 20: a circle becomes an ellipse, never a path, and is not counted.
#[test]
fn a_stretched_circle_becomes_an_ellipse_not_a_path() {
    let mut rig = Rig::new();
    let circle = rig.document.create_ellipse(EllipseFrame {
        center: pt(10.0, 10.0),
        rx: Length::from_mm(10.0),
        ry: Length::from_mm(10.0),
    });
    let rect = rig.rect(30.0, 0.0, 10.0, 20.0);
    rig.select(&[circle, rect]);
    let east = rig.east_edge();
    rig.drag(east, pt(east.x + 40.0, east.y), NONE);
    assert!(rig.document.primitive(circle).is_some(), "an ellipse");
    let ObjectSnapshot::Primitive(PrimitiveSnapshot {
        shape: Shape::Ellipse { frame },
        rotation,
        ..
    }) = rig.document.object(circle).unwrap()
    else {
        panic!("an ellipse");
    };
    assert!(
        (frame.rx.as_mm() - frame.ry.as_mm()).abs() > 1.0,
        "no longer a circle"
    );
    assert_eq!(rotation.as_radians(), 0.0);
    assert!(rig.tool.take_conversion_notice().is_empty());
}

/// Criteria 53.2 and 55: a rectangle turned by 30 degrees is a converting shape; an
/// edge stretch makes it a path with the same id, tree position and style.
#[test]
fn a_rotated_rectangle_becomes_a_path_in_place() {
    let mut rig = Rig::new();
    let first = rig.rect(0.0, 0.0, 10.0, 10.0);
    let turned = rig.rect(30.0, 0.0, 20.0, 8.0);
    let last = rig.rect(70.0, 0.0, 10.0, 10.0);
    rig.turned(turned, 30.0);
    rig.select(&[first, turned, last]);
    let before = rig.document.object(turned).unwrap();
    let order = rig.document.object_ids();
    let group = SelectTool::group_of(&rig.objects(), &rig.selection).unwrap();
    assert_eq!(group.converting().as_array(), [0, 0, 1, 0]);
    let east = rig.east_edge();
    rig.drag(east, pt(east.x + 30.0, east.y), NONE);
    assert!(is_path(&rig.document, turned));
    assert_eq!(rig.document.object_ids(), order);
    assert_eq!(rig.document.object(turned).unwrap().style(), before.style());
    assert!(rig.document.primitive(first).is_some() && rig.document.primitive(last).is_some());
    assert_eq!(rig.tool.take_conversion_notice().as_array(), [0, 0, 1, 0]);
}

/// Criterion 32: during the drag the converting shape is previewed as its path, the
/// release commits the same snapshots, and a drag back to equal factors previews
/// the shape again.
#[test]
fn the_preview_is_the_commit_and_a_drag_back_converts_nothing() {
    let (mut rig, star, rect) = star_and_rect();
    rig.select(&[star, rect]);
    let east = rig.east_edge();
    rig.press(east, NONE);
    let to = pt(east.x + 30.0, east.y);
    rig.tool.pointer_moved(to, NONE, &mut rig.selection);
    let live = rig.live(to, NONE).expect("a preview");
    assert!(
        matches!(live[0], ObjectSnapshot::Path(_)),
        "the star previews as a path"
    );
    assert!(matches!(live[1], ObjectSnapshot::Primitive(_)));
    assert_eq!(
        rig.tool.live_conversion_counts(to, false, false).as_array(),
        [0, 1, 0, 0]
    );
    // Back to the start: no preview, nothing converted.
    rig.tool.pointer_moved(east, NONE, &mut rig.selection);
    assert!(rig.live(east, NONE).is_none());
    assert!(
        rig.tool
            .live_conversion_counts(east, false, false)
            .is_empty()
    );
    // Out again and released: the committed objects are the previewed ones.
    rig.tool.pointer_moved(to, NONE, &mut rig.selection);
    let live = rig.live(to, NONE).unwrap();
    rig.release(to, NONE);
    assert_eq!(rig.objects(), live, "preview and release agree");
}

/// Criteria 53.4 and 54: Escape, a drag of factor 1 and a refused commit convert
/// nothing and leave no notice.
#[test]
fn escape_factor_one_and_a_refused_commit_leave_no_notice() {
    let (mut rig, star, rect) = star_and_rect();
    rig.select(&[star, rect]);
    let before = rig.objects();
    let east = rig.east_edge();
    // Escape.
    rig.press(east, NONE);
    rig.tool
        .pointer_moved(pt(east.x + 30.0, east.y), NONE, &mut rig.selection);
    rig.tool.escape();
    rig.release(pt(east.x + 30.0, east.y), NONE);
    assert_eq!(rig.objects(), before);
    assert!(rig.tool.take_conversion_notice().is_empty());
    // Dragged along the edge only: factor 1 on both axes.
    rig.drag(east, pt(east.x, east.y + 30.0), NONE);
    assert_eq!(rig.objects(), before);
    assert!(rig.tool.take_conversion_notice().is_empty());
    // A stale id: the snapshot holds an object that is gone, the document refuses it
    // all (criterion 53.6).
    let objects = rig.objects();
    rig.tool.pointer_down(
        &objects,
        &mut rig.selection,
        east,
        tol(),
        tolerances(),
        NONE,
    );
    rig.tool
        .pointer_moved(pt(east.x + 30.0, east.y), NONE, &mut rig.selection);
    rig.document.delete_objects(&[rect]).unwrap();
    rig.tool.pointer_up(
        &rig.document,
        &objects,
        &mut rig.selection,
        pt(east.x + 30.0, east.y),
        NONE,
        &mut AnchorIdMinter::new(9),
    );
    assert!(
        rig.document.primitive(star).is_some(),
        "the star is still a star"
    );
    assert!(rig.tool.take_conversion_notice().is_empty(), "no notice");
}

/// Criterion 53.4: move and rotate never convert.
#[test]
fn move_and_rotate_never_convert() {
    let (mut rig, star, rect) = star_and_rect();
    rig.select(&[star, rect]);
    let (min, max) = rig.group_box();
    // Rotate: the north-east corner rotate handle sits just outside the corner.
    let from = pt(max.x + 2.0, min.y - 2.0);
    rig.drag(from, pt(from.x - 8.0, from.y + 14.0), NONE);
    assert!(!is_path(&rig.document, star));
    // Move: the star's outline.
    let objects = rig.objects();
    let on_star = curvyo_ui_core::object_outline_bounds(&objects[0]).0;
    rig.drag(
        pt(on_star.x + 10.0, 30.0 + 0.0),
        pt(on_star.x + 25.0, 40.0),
        NONE,
    );
    assert!(!is_path(&rig.document, star));
    assert!(rig.tool.take_conversion_notice().is_empty());
}

/// Criterion 56: a single polygon or star shows all eight resize handles, its corner
/// drag is proportional, its edge drag turns it into a path.
#[test]
fn a_single_star_converts_on_an_edge_and_not_on_a_corner() {
    let mut rig = Rig::new();
    let star = rig.star(0.0, 10.0);
    rig.select(&[star]);
    let objects = rig.objects();
    let handles = SelectTool::transform_handles(&objects, &rig.selection, tolerances(), false);
    let resize = handles
        .iter()
        .filter(|(h, _)| matches!(h, EditHandle::Resize(_)))
        .count();
    assert_eq!(resize, 8);
    let position = |direction| {
        handles
            .iter()
            .find(|(h, _)| *h == EditHandle::Resize(direction))
            .map(|(_, p)| *p)
            .unwrap()
    };
    // The corner: proportional, still a star, no notice.
    let se = position(ResizeDirection::Se);
    rig.drag(se, pt(se.x + 12.0, se.y + 2.0), NONE);
    assert!(rig.document.primitive(star).is_some());
    assert!(rig.tool.take_conversion_notice().is_empty());
    // The east edge: a stretch.
    let objects = rig.objects();
    let handles = SelectTool::transform_handles(&objects, &rig.selection, tolerances(), false);
    let east = handles
        .iter()
        .find(|(h, _)| *h == EditHandle::Resize(ResizeDirection::E))
        .map(|(_, p)| *p)
        .unwrap();
    rig.drag(east, pt(east.x + 30.0, east.y), NONE);
    assert!(
        is_path(&rig.document, star),
        "an edge stretch turns it into a path"
    );
    assert_eq!(rig.tool.take_conversion_notice().as_array(), [0, 1, 0, 0]);
}

/// Criterion 56: the typed size of a single polygon has W and H; a size of another
/// aspect ratio converts it, the same aspect keeps it.
#[test]
fn a_single_polygon_typed_size_converts_only_when_it_is_a_stretch() {
    let mut rig = Rig::new();
    let polygon = rig.document.create_polygon(
        StarFrame {
            center: pt(30.0, 30.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(6).unwrap(),
    );
    rig.select(&[polygon]);
    let objects = rig.objects();
    rig.tool
        .open_entry_for_key(&objects, &rig.selection, EntryKey::Size)
        .unwrap();
    rig.tool.mint_conversion_ids(&mut AnchorIdMinter::new(9));
    let entry = rig.tool.entry().unwrap();
    assert_eq!(entry.fields().len(), 2);
    let (w, h) = (
        entry.fields()[0].prefill.clone(),
        entry.fields()[1].prefill.clone(),
    );
    // Same aspect ratio (both times two): a uniform scale.
    let outcome = rig.tool.commit_entry(&rig.document, ["40.0", "40.0"], 0);
    assert_eq!(outcome, EntryOutcome::Committed);
    assert!(
        rig.document.primitive(polygon).is_some(),
        "still a polygon ({w} x {h})"
    );
    assert!(rig.tool.take_conversion_notice().is_empty());
    // Another aspect: a stretch.
    let objects = rig.objects();
    rig.tool
        .open_entry_for_key(&objects, &rig.selection, EntryKey::Size)
        .unwrap();
    rig.tool.mint_conversion_ids(&mut AnchorIdMinter::new(9));
    let outcome = rig.tool.commit_entry(&rig.document, ["60.0", "40.0"], 0);
    assert_eq!(outcome, EntryOutcome::Committed);
    assert!(is_path(&rig.document, polygon));
    assert_eq!(rig.tool.take_conversion_notice().as_array(), [1, 0, 0, 0]);
}

/// Criteria 34 and 55: the typed size of a selection is a stretch only when the
/// aspect ratio changes; the entry says how many shapes it would convert.
#[test]
fn the_group_size_entry_counts_what_a_stretch_would_convert() {
    let (mut rig, star, rect) = star_and_rect();
    rig.select(&[star, rect]);
    let objects = rig.objects();
    rig.tool
        .open_entry_for_key(&objects, &rig.selection, EntryKey::Size)
        .unwrap();
    let entry = rig.tool.group_entry().unwrap();
    let (w, h) = (
        entry.fields()[0].prefill.clone(),
        entry.fields()[1].prefill.clone(),
    );
    assert!(entry.conversion_counts([&w, &h], 0).is_empty(), "unedited");
    assert!(
        entry.conversion_counts(["90.0", "40.0"], 0).as_array() == [0, 1, 0, 0],
        "a wider box converts the star"
    );
    let h2: f64 = h.parse::<f64>().unwrap() * 2.0;
    let w2: f64 = w.parse::<f64>().unwrap() * 2.0;
    let _ = (w2, h2);
    // Exactly twice both ways (the prefill is rounded; the box itself is not).
    let (exact_w, exact_h) = {
        let group = SelectTool::group_of(&rig.objects(), &rig.selection).unwrap();
        (group.bounds().width() * 2.0, group.bounds().height() * 2.0)
    };
    assert!(
        entry
            .conversion_counts([&format!("{exact_w}"), &format!("{exact_h}")], 0)
            .is_empty(),
        "twice both ways is a uniform scale"
    );
    assert_eq!(
        rig.tool.commit_entry(&rig.document, ["90.0", &h], 0),
        EntryOutcome::Committed
    );
    assert!(is_path(&rig.document, star));
    assert_eq!(rig.tool.take_conversion_notice().as_array(), [0, 1, 0, 0]);
}

/// Decision 8: the ids of a converted path are the session's own once minted; two
/// drags never give two anchors the same id.
#[test]
fn converted_anchor_ids_come_from_the_session_minter() {
    let (mut rig, star, rect) = star_and_rect();
    rig.select(&[star, rect]);
    let east = rig.east_edge();
    let objects = rig.objects();
    rig.tool.pointer_down(
        &objects,
        &mut rig.selection,
        east,
        tol(),
        tolerances(),
        NONE,
    );
    let mut minter = AnchorIdMinter::new(77);
    rig.tool.mint_conversion_ids(&mut minter);
    rig.tool
        .pointer_moved(pt(east.x + 20.0, east.y), NONE, &mut rig.selection);
    let live = rig.live(pt(east.x + 20.0, east.y), NONE).unwrap();
    let ObjectSnapshot::Path(path) = &live[0] else {
        panic!("a path");
    };
    let peer_of = |a: &curvyo_document_core::AnchorSnapshot| a.id.to_hex()[..16].to_string();
    assert!(
        path.anchors
            .iter()
            .all(|a| peer_of(a) == format!("{:016x}", 77))
    );
    let mut ids: Vec<_> = path.anchors.iter().map(|a| a.id).collect();
    ids.sort_by_key(|id| id.to_hex());
    ids.dedup();
    assert_eq!(ids.len(), 10, "ten different ids");
    // The session's counter moved on by the ten it minted.
    assert_eq!(minter.mint().to_hex()[16..], format!("{:016x}", 10));
}

/// Criterion 34: a typed width or height above 10,000,000 mm is "Too large", not
/// "Enter a number", for one object and for a selection alike.
#[test]
fn a_typed_size_beyond_the_limit_is_too_large() {
    use curvyo_ui_core::InvalidReason;
    for ids in [1usize, 2] {
        let mut rig = Rig::new();
        let a = rig.rect(0.0, 0.0, 40.0, 20.0);
        let b = rig.rect(60.0, 0.0, 40.0, 20.0);
        let chosen = if ids == 1 { vec![a] } else { vec![a, b] };
        rig.select(&chosen);
        let objects = rig.objects();
        rig.tool
            .open_entry_for_key(&objects, &rig.selection, EntryKey::Size)
            .unwrap();
        for (texts, field) in [(["20000000", "20"], 0usize), (["40", "99999999"], 1)] {
            assert_eq!(
                rig.tool.commit_entry(&rig.document, texts, field),
                EntryOutcome::Invalid {
                    field,
                    reason: InvalidReason::TooLarge
                },
                "{ids} object(s), {texts:?}"
            );
        }
    }
}
