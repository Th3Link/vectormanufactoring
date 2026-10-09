//! A compound path in the Select tool (`specs/0016-boolean-operations`
//! criteria 33 to 36a, 38): hit-testing through a hole, the selection box over
//! all outlines, move, resize, rotate, skew and copy of every outline
//! together, and no handoff to the Node tool.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]

use std::collections::HashSet;

use curvyo_document_core::{
    AnchorId, Angle, Color, Document, FillMode, FillModeTarget, Length, NewAnchor, NodeId,
    ObjectSnapshot, PathSnapshot, Point, RectBounds, StyleEdit, Tolerance, Vec2,
};
use curvyo_ui_core::{
    AnchorIdMinter, EditHandle, EntryOutcome, Modifiers, NodeSelection, ObjectSelection,
    ResizeDirection, SelectDoubleClickOutcome, SelectTool, Side, StrokeScaling, StyleTool,
    TransformHandleTolerances, fit_document_to_content, hit_test_object, hit_test_objects_along,
    object_bounds, object_outline_bounds, oriented_bounds, style_scope,
};

const SEGMENT_TOLERANCE: Tolerance = Tolerance::from_mm(0.5);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(2.0)
}

fn square(first: u64, x: f64, y: f64, side: f64, reversed: bool) -> (Vec<NewAnchor>, bool) {
    let mut corners = vec![(x, y), (x + side, y), (x + side, y + side), (x, y + side)];
    if reversed {
        corners.reverse();
    }
    let anchors = corners
        .into_iter()
        .enumerate()
        .map(|(k, (cx, cy))| NewAnchor::corner(AnchorId::new(4, first + k as u64), pt(cx, cy)))
        .collect();
    (anchors, true)
}

/// A compound path of `outlines`, filled solid, stroke width 1.
fn add_compound(document: &Document, outlines: &[(Vec<NewAnchor>, bool)]) -> NodeId {
    let seed = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(9, 1), pt(900.0, 900.0)),
            NewAnchor::corner(AnchorId::new(9, 2), pt(901.0, 900.0)),
        ],
        false,
    );
    let id = document
        .replace_with_path(&[seed], seed, outlines, "boolean_difference")
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(Length::from_mm(1.0)))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::FillColor(Color { r: 9, g: 9, b: 9 }))
        .unwrap();
    document
        .set_fill_mode(
            FillMode::Solid,
            &[FillModeTarget {
                id,
                seed_stops: vec![],
            }],
        )
        .unwrap();
    id
}

fn ring_outlines() -> Vec<(Vec<NewAnchor>, bool)> {
    vec![
        square(100, 0.0, 0.0, 40.0, false),
        square(200, 10.0, 10.0, 20.0, true),
    ]
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn path_of(document: &Document, id: NodeId) -> PathSnapshot {
    document.path(id).unwrap()
}

// ------------------------------------------------------------------ hit test

/// Criterion 33: a click inside the hole picks what is behind (or nothing); a
/// click on the fill between the outlines, or on the stroke of any outline,
/// picks the compound path.
#[test]
fn a_click_in_the_hole_misses_and_a_click_on_the_fill_or_any_stroke_hits() {
    let document = Document::new(1);
    let behind = document.create_rect(RectBounds {
        origin: pt(5.0, 5.0),
        width: Length::from_mm(30.0),
        height: Length::from_mm(30.0),
    });
    document
        .set_fill_mode(
            FillMode::Solid,
            &[FillModeTarget {
                id: behind,
                seed_stops: vec![],
            }],
        )
        .unwrap();
    let ring = add_compound(&document, &ring_outlines());
    let objects = objects(&document);
    let pick = |x: f64, y: f64| hit_test_object(&objects, pt(x, y), SEGMENT_TOLERANCE);

    assert_eq!(
        pick(20.0, 20.0),
        Some(behind),
        "in the hole: what is behind"
    );
    assert_eq!(
        pick(7.0, 20.0),
        Some(ring),
        "on the fill between the outlines"
    );
    assert_eq!(
        pick(10.0, 20.0),
        Some(ring),
        "on the stroke of the inner outline"
    );
    assert_eq!(
        pick(0.0, 20.0),
        Some(ring),
        "on the stroke of the outer outline"
    );
    assert_eq!(pick(60.0, 60.0), None, "away from everything");

    // With nothing behind, the hole picks nothing.
    let alone = Document::new(2);
    let _ = add_compound(&alone, &ring_outlines());
    assert_eq!(
        hit_test_object(&objects_of(&alone), pt(20.0, 20.0), SEGMENT_TOLERANCE),
        None
    );
}

fn objects_of(document: &Document) -> Vec<ObjectSnapshot> {
    objects(document)
}

/// The stroke of an outline far from the first picks the object too.
#[test]
fn the_stroke_of_a_separate_piece_picks_the_compound_path() {
    let document = Document::new(1);
    let id = add_compound(
        &document,
        &[
            square(100, 0.0, 0.0, 10.0, false),
            square(200, 100.0, 0.0, 10.0, false),
        ],
    );
    let objects = objects(&document);
    assert_eq!(
        hit_test_object(&objects, pt(100.0, 5.0), SEGMENT_TOLERANCE),
        Some(id)
    );
    assert_eq!(
        hit_test_object(&objects, pt(50.0, 5.0), SEGMENT_TOLERANCE),
        None
    );
}

// -------------------------------------------------------------------- bounds

/// Criterion 34: the selection box is the box of all outlines, rotated
/// registers included.
#[test]
fn the_selection_box_covers_every_outline() {
    let document = Document::new(1);
    let id = add_compound(
        &document,
        &[
            square(100, 0.0, 0.0, 10.0, false),
            square(200, 100.0, 50.0, 20.0, false),
        ],
    );
    let object = document.object(id).unwrap();
    for (min, max) in [object_bounds(&object), object_outline_bounds(&object)] {
        assert_eq!((min, max), (pt(0.0, 0.0), pt(120.0, 70.0)));
    }
    let oriented = oriented_bounds(&object);
    assert_eq!(oriented.min, pt(0.0, 0.0));
    assert_eq!(oriented.max, pt(120.0, 70.0));

    // Rotated by 90 degrees about the centre: the register is named in the
    // box, and the box is still the box of every outline in its own frame.
    let turned = object.rotated(
        pt(60.0, 35.0),
        Angle::from_radians(std::f64::consts::FRAC_PI_2),
    );
    document.rotate_object(&turned).unwrap();
    let oriented = oriented_bounds(&document.object(id).unwrap());
    let width = oriented.max.x - oriented.min.x;
    let height = oriented.max.y - oriented.min.y;
    assert!((width - 120.0).abs() < 1e-6 && (height - 70.0).abs() < 1e-6);
}

// ------------------------------------------------------- transforms by gesture

struct Rig {
    document: Document,
    id: NodeId,
    selection: ObjectSelection,
    tool: SelectTool,
    minter: AnchorIdMinter,
}

impl Rig {
    fn ring() -> Self {
        let document = Document::new(1);
        let id = add_compound(&document, &ring_outlines());
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        Self {
            document,
            id,
            selection,
            tool: SelectTool::new(),
            minter: AnchorIdMinter::new(77),
        }
    }

    fn path(&self) -> PathSnapshot {
        path_of(&self.document, self.id)
    }

    fn handle(&self, wanted: EditHandle) -> Point {
        SelectTool::transform_handles(
            &objects(&self.document),
            &self.selection,
            tolerances(),
            false,
        )
        .into_iter()
        .find(|(h, _)| *h == wanted)
        .unwrap_or_else(|| panic!("{wanted:?} is drawn"))
        .1
    }

    fn drag(&mut self, from: Point, to: Point, modifiers: Modifiers) {
        let objects = objects(&self.document);
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            from,
            SEGMENT_TOLERANCE,
            tolerances(),
            Modifiers::NONE,
        );
        self.tool
            .pointer_moved(to, Modifiers::NONE, &mut self.selection);
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            to,
            modifiers,
            &mut self.minter,
        );
    }

    fn typed(&mut self, handle: EditHandle, texts: [&str; 2]) {
        let at = self.handle(handle);
        let objects = objects(&self.document);
        for _ in 0..2 {
            // A double-click: the first press must have grabbed the handle.
            self.tool.pointer_down(
                &objects,
                &mut self.selection,
                at,
                SEGMENT_TOLERANCE,
                tolerances(),
                Modifiers::NONE,
            );
            self.tool.pointer_up(
                &self.document,
                &objects,
                &mut self.selection,
                at,
                Modifiers::NONE,
                &mut self.minter,
            );
        }
        let outcome = self.tool.double_click(
            &objects,
            &self.selection,
            at,
            SEGMENT_TOLERANCE,
            tolerances(),
            (false, false),
        );
        assert_eq!(outcome, SelectDoubleClickOutcome::EntryOpened);
        assert_eq!(
            self.tool.commit_entry(&self.document, texts, 0),
            EntryOutcome::Committed
        );
    }
}

/// The centre of the hole is not painted: no object is hit there.
fn centre_is_a_hole(rig: &Rig) -> bool {
    let path = rig.path();
    let hole: Vec<Point> = path.extra_subpaths[0]
        .anchors
        .iter()
        .map(|a| a.point)
        .collect();
    let centre = Point::new(
        hole.iter().map(|p| p.x).sum::<f64>() / 4.0,
        hole.iter().map(|p| p.y).sum::<f64>() / 4.0,
    );
    hit_test_object(&objects(&rig.document), centre, Tolerance::from_mm(0.01)).is_none()
}

/// Criterion 35: a drag move shifts every outline by the same offset, in one
/// commit.
#[test]
fn a_move_shifts_every_outline() {
    let mut rig = Rig::ring();
    let before = rig.path();
    // Press on the fill between the outlines (x = 5), drag by (30, 7).
    rig.drag(pt(5.0, 20.0), pt(35.0, 27.0), Modifiers::NONE);
    let after = rig.path();
    for (new, old) in after.all_anchors().zip(before.all_anchors()) {
        assert_eq!(new.point, old.point.translated(Vec2::new(30.0, 7.0)));
        assert_eq!(new.id, old.id);
    }
    assert!(centre_is_a_hole(&rig));
}

/// Criterion 35: resize scales every outline; holes stay holes; the stroke
/// width is scaled once when the switch is on (35a).
#[test]
fn a_resize_scales_every_outline_and_the_stroke_width_once() {
    let mut rig = Rig::ring();
    rig.tool.set_stroke_scaling(StrokeScaling::Proportional);
    let before = rig.path();
    let se = rig.handle(EditHandle::Resize(ResizeDirection::Se));
    // Anchored at the top-left (0, 0): x doubles, y stays.
    rig.drag(se, pt(se.x + 40.0, se.y), Modifiers::NONE);
    let after = rig.path();
    for (new, old) in after.all_anchors().zip(before.all_anchors()) {
        assert!((new.point.x - old.point.x * 2.0).abs() < 1e-6, "{new:?}");
        assert!((new.point.y - old.point.y).abs() < 1e-6);
    }
    assert!(centre_is_a_hole(&rig));
    let ratio = after.style.stroke.width.as_mm() / before.style.stroke.width.as_mm();
    assert!(
        (ratio - 2.0_f64.sqrt()).abs() < 0.5 && ratio > 1.0,
        "scaled once by the box's factor, not once per outline: {ratio}"
    );
    // Once per object: the width of a one-outline path of the same box scales
    // by the same factor.
    let single = Document::new(3);
    let id = add_compound(&single, &[square(100, 0.0, 0.0, 40.0, false)]);
    let mut selection = ObjectSelection::new();
    selection.select_single(id);
    let mut tool = SelectTool::new();
    tool.set_stroke_scaling(StrokeScaling::Proportional);
    let objects = objects(&single);
    let se = SelectTool::transform_handles(&objects, &selection, tolerances(), false)
        .into_iter()
        .find(|(h, _)| *h == EditHandle::Resize(ResizeDirection::Se))
        .unwrap()
        .1;
    tool.pointer_down(
        &objects,
        &mut selection,
        se,
        SEGMENT_TOLERANCE,
        tolerances(),
        Modifiers::NONE,
    );
    tool.pointer_moved(pt(se.x + 40.0, se.y), Modifiers::NONE, &mut selection);
    tool.pointer_up(
        &single,
        &objects,
        &mut selection,
        pt(se.x + 40.0, se.y),
        Modifiers::NONE,
        &mut AnchorIdMinter::new(5),
    );
    let single_width = path_of(&single, id).style.stroke.width.as_mm();
    assert!((single_width - after.style.stroke.width.as_mm()).abs() < 1e-9);
}

/// Criterion 35: rotate by 37 degrees and scale to 150 % by 50 % (typed
/// values); the point that was the ring's centre is still not painted, and
/// every outline moved.
#[test]
fn a_typed_rotation_and_a_typed_resize_keep_the_hole() {
    let mut rig = Rig::ring();
    let before = rig.path();
    rig.typed(EditHandle::Rotate(ResizeDirection::Ne), ["37", ""]);
    let rotated = rig.path();
    assert!((rotated.rotation.as_radians().to_degrees() - 37.0).abs() < 1e-9);
    assert!(
        rotated
            .all_anchors()
            .zip(before.all_anchors())
            .all(|(a, b)| a.point != b.point)
    );
    assert!(centre_is_a_hole(&rig));

    rig.typed(EditHandle::Resize(ResizeDirection::Se), ["60", "20"]);
    let scaled = rig.path();
    assert_eq!(scaled.all_anchors().count(), 8);
    assert!(centre_is_a_hole(&rig), "still not painted after the resize");
    // The ring is a 40 x 40 box scaled to 60 x 20: 150 % by 50 %. The hole's
    // anchor ids are unchanged and its corners moved with the rest.
    assert_eq!(
        scaled.extra_subpaths[0]
            .anchors
            .iter()
            .map(|a| a.id)
            .collect::<Vec<_>>(),
        before.extra_subpaths[0]
            .anchors
            .iter()
            .map(|a| a.id)
            .collect::<Vec<_>>()
    );
}

/// Criterion 35: a skew moves every outline (a shear along the box edge).
#[test]
fn a_skew_moves_every_outline() {
    let mut rig = Rig::ring();
    let before = rig.path();
    let top = rig.handle(EditHandle::Skew(Side::Top));
    rig.drag(top, pt(top.x + 8.0, top.y), Modifiers::NONE);
    let after = rig.path();
    let moved = |a: &PathSnapshot, b: &PathSnapshot| {
        a.all_anchors()
            .zip(b.all_anchors())
            .filter(|(x, y)| x.point != y.point)
            .count()
    };
    assert!(
        moved(&after, &before) >= 6,
        "outer and hole corners shear, only the fixed line stays"
    );
    assert!(centre_is_a_hole(&rig));
}

/// Criterion 36, 36a: a copy by Ctrl-move has the same outlines, every anchor
/// a new unique id, and the original keeps its ids.
#[test]
fn a_ctrl_move_copies_every_outline_with_fresh_ids() {
    let mut rig = Rig::ring();
    let original = rig.path();
    rig.drag(pt(5.0, 20.0), pt(105.0, 20.0), Modifiers::new(false, true));
    let ids = rig.document.object_ids();
    assert_eq!(ids.len(), 2);
    assert_eq!(rig.path(), original, "the original stays");
    let copy = path_of(&rig.document, ids[1]);
    assert_eq!(copy.extra_subpaths.len(), 1);
    let all: Vec<AnchorId> = original
        .all_anchors()
        .chain(copy.all_anchors())
        .map(|a| a.id)
        .collect();
    assert_eq!(all.iter().collect::<HashSet<_>>().len(), 16, "unique ids");
    for (new, old) in copy.all_anchors().zip(original.all_anchors()) {
        assert_eq!(new.point, old.point.translated(Vec2::new(100.0, 0.0)));
    }
}

/// Criterion 38: a double-click on a compound path stays in the Select tool.
#[test]
fn a_double_click_on_a_compound_path_does_not_hand_off_to_the_node_tool() {
    let mut rig = Rig::ring();
    // The stroke of the hole, away from every handle of the box.
    let at = pt(10.0, 15.0);
    let objects = objects(&rig.document);
    rig.tool.pointer_down(
        &objects,
        &mut rig.selection,
        at,
        SEGMENT_TOLERANCE,
        tolerances(),
        Modifiers::NONE,
    );
    rig.tool.pointer_up(
        &rig.document,
        &objects,
        &mut rig.selection,
        at,
        Modifiers::NONE,
        &mut rig.minter,
    );
    let outcome = rig.tool.double_click(
        &objects,
        &rig.selection,
        at,
        SEGMENT_TOLERANCE,
        tolerances(),
        (false, false),
    );
    assert_eq!(outcome, SelectDoubleClickOutcome::CompoundPath);
}

// ------------------------------------------------------------ document fit

/// Rulers criterion 23 with a compound path: the document fits the box of
/// every outline, so a piece far from the first outline is not left out, and
/// the objects move so the box starts at (0, 0).
#[test]
fn fit_to_content_covers_every_outline_of_a_compound_path() {
    let document = Document::new(1);
    let id = add_compound(
        &document,
        &[
            square(100, 20.0, 30.0, 10.0, false),
            square(200, 120.0, 80.0, 30.0, false),
        ],
    );
    assert_eq!(fit_document_to_content(&document), Ok(true));
    let size = document.size();
    // The box runs from (20, 30) to (150, 110): 130 x 80 mm.
    assert!((size.width.as_mm() - 130.0).abs() < 1e-9, "{size:?}");
    assert!((size.height.as_mm() - 80.0).abs() < 1e-9, "{size:?}");
    let path = path_of(&document, id);
    assert_eq!(
        path.anchors[0].point,
        pt(0.0, 0.0),
        "the first piece moved by (-20, -30)"
    );
    assert_eq!(path.extra_subpaths[0].anchors[2].point, pt(130.0, 80.0));
    assert_eq!(
        fit_document_to_content(&document),
        Ok(false),
        "already fits"
    );
}

/// The lasso picks a compound path by the outline of any of its outlines, and a line that stays
/// in the hole (touching no outline) picks nothing.
#[test]
fn a_lasso_picks_a_compound_path_by_any_of_its_outlines() {
    let document = Document::new(1);
    let id = add_compound(&document, &ring_outlines());
    let objects = objects(&document);
    let tolerance = Tolerance::from_mm(0.5);
    let in_the_hole = [pt(14.0, 20.0), pt(26.0, 20.0)];
    assert_eq!(
        hit_test_objects_along(&objects, &in_the_hole, tolerance),
        Vec::new()
    );
    let across_the_hole_wall = [pt(5.0, 20.0), pt(14.0, 20.0)];
    assert_eq!(
        hit_test_objects_along(&objects, &across_the_hole_wall, tolerance),
        vec![id]
    );
    let outside_touching_outer = [pt(-5.0, 20.0), pt(2.0, 20.0)];
    assert_eq!(
        hit_test_objects_along(&objects, &outside_touching_outer, tolerance),
        vec![id]
    );
}

// ------------------------------------------------------------ subject line

/// The Properties panel names a compound path as such (criterion 38), one or several.
#[test]
fn the_subject_line_names_compound_paths() {
    let document = Document::new(1);
    let first = add_compound(&document, &ring_outlines());
    let objects_one = objects(&document);
    let mut selection = ObjectSelection::new();
    selection.set(&[first]);
    let one = style_scope(
        StyleTool::Other,
        &objects_one,
        &selection,
        &NodeSelection::new(),
    );
    assert_eq!(one.subject, "Compound path");

    let rect = document.create_rect(RectBounds {
        origin: pt(500.0, 0.0),
        width: Length::from_mm(5.0),
        height: Length::from_mm(5.0),
    });
    let all = objects(&document);
    selection.set(&[first, rect]);
    let mixed = style_scope(StyleTool::Other, &all, &selection, &NodeSelection::new());
    assert_eq!(mixed.subject, "2 objects");

    // Compound paths are paths: with ordinary paths they are "N paths", among themselves
    // "N compound paths".
    let plain = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(8, 1), pt(700.0, 0.0)),
            NewAnchor::corner(AnchorId::new(8, 2), pt(710.0, 0.0)),
            NewAnchor::corner(AnchorId::new(8, 3), pt(710.0, 10.0)),
        ],
        true,
    );
    let second = add_compound(
        &document,
        &[
            square(300, 0.0, 100.0, 40.0, false),
            square(400, 10.0, 110.0, 20.0, true),
        ],
    );
    let all = objects(&document);
    let mut subject = |ids: &[NodeId]| {
        selection.set(ids);
        style_scope(StyleTool::Other, &all, &selection, &NodeSelection::new()).subject
    };
    assert_eq!(subject(&[first, plain]), "2 paths");
    assert_eq!(subject(&[first, second]), "2 compound paths");
    assert_eq!(subject(&[first, plain, rect]), "3 objects");
}
