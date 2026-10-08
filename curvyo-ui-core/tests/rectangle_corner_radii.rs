//! `curvyo-ui-core`'s share of PR 2 of `specs/0013-rectangle-corner-radii/`: the
//! "Link corners" switch and Shift (criteria 2 and 3), the per-corner drag and
//! its limit (4, 5), the typed entry's scope (6), Remove rounding (8) and the
//! drag facts for the readout and the follower knobs (7, 23). The pure value
//! rules are unit-tested in `param_edit.rs`; this drives the real tool.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    CornerRadii, Document, Length, NodeId, ObjectSnapshot, Point, PrimitiveSnapshot, RectBounds,
    Shape, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, Corner, CornerLinking, EditHandle, EntryOutcome, Modifiers, ObjectSelection,
    ParamHandle, SelectDoubleClickOutcome, SelectTool, TransformHandleTolerances,
};

const SEGMENT_TOLERANCE: Tolerance = Tolerance::from_mm(1.0);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Two screen pixels per millimetre.
fn tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(2.0)
}

fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
    CornerRadii {
        tl: Length::from_mm(tl),
        tr: Length::from_mm(tr),
        br: Length::from_mm(br),
        bl: Length::from_mm(bl),
    }
}

struct Rig {
    document: Document,
    id: NodeId,
    selection: ObjectSelection,
    tool: SelectTool,
}

impl Rig {
    /// A 100 x 60 mm rectangle with the given stored radii, selected.
    fn new(stored: CornerRadii) -> Self {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: pt(10.0, 20.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(60.0),
        });
        document.set_corner_radii(&[(id, stored)]).unwrap();
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

    fn knob(&self, corner: Corner) -> Point {
        let wanted = EditHandle::Param(ParamHandle::CornerRadius(corner));
        SelectTool::transform_handles(&self.objects(), &self.selection, tolerances(), false)
            .into_iter()
            .find(|(handle, _)| *handle == wanted)
            .expect("the knob is drawn")
            .1
    }

    fn press(&mut self, at: Point, shift: bool) {
        let objects = self.objects();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            at,
            SEGMENT_TOLERANCE,
            tolerances(),
            Modifiers::new(shift, false),
        );
    }

    fn release(&mut self, at: Point) {
        let objects = self.objects();
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            at,
            Modifiers::NONE,
            &mut AnchorIdMinter::new(99),
        );
    }

    /// Presses on `corner`'s knob (with `shift`), drags it `delta` along its
    /// inward diagonal in millimetres of document space and releases.
    fn drag(&mut self, corner: Corner, shift: bool, delta: f64) {
        let from = self.knob(corner);
        self.press(from, shift);
        self.release(along(from, corner, delta));
    }

    fn stored(&self) -> CornerRadii {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radii, .. },
            ..
        }) = self.objects().remove(0)
        else {
            panic!("a rectangle");
        };
        corner_radii
    }
}

/// `from` moved by `delta` mm along the inward diagonal of `corner`.
fn along(from: Point, corner: Corner, delta: f64) -> Point {
    let diagonal = corner.inward_diagonal();
    pt(from.x + diagonal.x * delta, from.y + diagonal.y * delta)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Criterion 2: linked by default; setting the switch changes no radius and
/// writes nothing.
#[test]
fn the_switch_starts_linked_and_toggling_it_writes_nothing() {
    let mut rig = Rig::new(radii(1.0, 2.0, 3.0, 4.0));
    assert_eq!(rig.tool.corner_linking(), CornerLinking::Linked);
    let before = rig.document.export_loro_snapshot().unwrap().len();
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    rig.tool.set_corner_linking(CornerLinking::Linked);
    assert_eq!(rig.stored(), radii(1.0, 2.0, 3.0, 4.0));
    assert_eq!(rig.document.export_loro_snapshot().unwrap().len(), before);
}

/// Criteria 2 and 3: the drag is unlinked when the switch and Shift differ.
#[test]
fn shift_inverts_the_switch_for_one_drag() {
    for (linking, shift, one_corner) in [
        (CornerLinking::Linked, false, false),
        (CornerLinking::Linked, true, true),
        (CornerLinking::Unlinked, false, true),
        (CornerLinking::Unlinked, true, false),
    ] {
        assert_eq!(linking.is_unlinked_with(shift), one_corner);
        let mut rig = Rig::new(radii(5.0, 0.0, 12.0, 3.0));
        rig.tool.set_corner_linking(linking);
        rig.drag(Corner::Tl, shift, 10.0);
        let after = rig.stored();
        if one_corner {
            assert_eq!(
                (after.tr, after.br, after.bl),
                (
                    radii(0.0, 0.0, 12.0, 3.0).tr,
                    radii(0.0, 0.0, 12.0, 3.0).br,
                    radii(0.0, 0.0, 12.0, 3.0).bl
                )
            );
            assert!(after.tl.as_mm() > 5.0, "{linking:?} shift {shift}");
        } else {
            assert_eq!(
                after,
                CornerRadii::uniform(after.tl),
                "{linking:?} shift {shift}"
            );
            assert!(after.tl.as_mm() > 5.0);
        }
    }
}

/// Criterion 3: the state is read once at the press; a click on the switch
/// mid-drag changes nothing until the next drag.
#[test]
fn the_state_is_frozen_at_the_press() {
    let mut rig = Rig::new(radii(5.0, 0.0, 12.0, 3.0));
    let from = rig.knob(Corner::Tl);
    rig.press(from, false); // linked
    rig.tool.set_corner_linking(CornerLinking::Unlinked); // mid-drag
    rig.release(along(from, Corner::Tl, 10.0));
    let after = rig.stored();
    assert_eq!(
        after,
        CornerRadii::uniform(after.tl),
        "the drag stayed linked"
    );

    // And the other way: pressed unlinked, switched back mid-drag.
    let mut rig = Rig::new(radii(5.0, 0.0, 12.0, 3.0));
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    let from = rig.knob(Corner::Tl);
    rig.press(from, false);
    rig.tool.set_corner_linking(CornerLinking::Linked);
    rig.release(along(from, Corner::Tl, 10.0));
    let after = rig.stored();
    assert_eq!(
        (after.tr.as_mm(), after.br.as_mm(), after.bl.as_mm()),
        (0.0, 12.0, 3.0)
    );
}

/// Criterion 3: the other three radii do not change at any moment of an
/// unlinked drag, in the live preview either.
#[test]
fn an_unlinked_drag_previews_one_corner_only() {
    let mut rig = Rig::new(radii(5.0, 8.0, 12.0, 3.0));
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    let from = rig.knob(Corner::Br);
    rig.press(from, false);
    for step in [-5.0, 0.0, 4.0, 9.0, 30.0] {
        let live = rig.tool.live_edit(
            &rig.objects(),
            &rig.selection,
            along(from, Corner::Br, step),
            false,
            false,
        );
        let Some(live) = live else { continue };
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radii, .. },
            ..
        }) = &live.objects[0]
        else {
            panic!("a rectangle");
        };
        assert_eq!(
            (corner_radii.tl, corner_radii.tr, corner_radii.bl),
            (
                Length::from_mm(5.0),
                Length::from_mm(8.0),
                Length::from_mm(3.0)
            ),
            "step {step}"
        );
    }
}

/// Criterion 4: the dragged corner stops where the neighbours' effective radii
/// leave no room: `min(W - r_h, H - r_v)`.
#[test]
fn an_unlinked_drag_stops_at_the_limit() {
    let mut rig = Rig::new(radii(0.0, 40.0, 0.0, 30.0));
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    rig.drag(Corner::Tl, false, 500.0);
    // min(100 - 40, 60 - 30) = 30.
    assert!(close(rig.stored().tl.as_mm(), 30.0), "{:?}", rig.stored());
    assert_eq!(rig.stored().tr, Length::from_mm(40.0));
}

/// Criterion 5: back to the zero position is exactly 0, for that corner only
/// when unlinked.
#[test]
fn dragging_back_to_zero_is_exactly_zero() {
    let mut rig = Rig::new(radii(5.0, 8.0, 12.0, 3.0));
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    rig.drag(Corner::Br, false, -500.0);
    assert_eq!(rig.stored(), radii(5.0, 8.0, 0.0, 3.0));
}

/// Criterion 7: Escape cancels the drag and writes nothing.
#[test]
fn escape_writes_nothing() {
    let mut rig = Rig::new(radii(5.0, 8.0, 12.0, 3.0));
    let from = rig.knob(Corner::Tl);
    rig.press(from, false);
    let _ = rig.tool.live_edit(
        &rig.objects(),
        &rig.selection,
        along(from, Corner::Tl, 10.0),
        false,
        false,
    );
    rig.tool.escape();
    rig.release(along(from, Corner::Tl, 10.0));
    assert_eq!(rig.stored(), radii(5.0, 8.0, 12.0, 3.0));
}

/// Criteria 7 and 23: the readout facts and the follower rule of a drag.
#[test]
fn the_drag_facts_say_max_all_corners_and_who_follows() {
    // Linked drag on unequal radii: it overwrites them, the followers move.
    let mut rig = Rig::new(radii(5.0, 8.0, 12.0, 3.0));
    let from = rig.knob(Corner::Tl);
    rig.press(from, false);
    assert_eq!(rig.tool.corner_drag_changes_all(), Some(true));
    let info = rig
        .tool
        .live_param_drag(along(from, Corner::Tl, 6.0))
        .unwrap();
    assert!(info.overwrites_unequal && !info.limited);
    let info = rig
        .tool
        .live_param_drag(along(from, Corner::Tl, 500.0))
        .unwrap();
    assert!(info.limited, "half the shorter side stops a linked drag");
    rig.tool.escape();

    // Linked drag on equal radii: no "all corners".
    let mut rig = Rig::new(CornerRadii::uniform(Length::from_mm(5.0)));
    let from = rig.knob(Corner::Tl);
    rig.press(from, false);
    let info = rig
        .tool
        .live_param_drag(along(from, Corner::Tl, 6.0))
        .unwrap();
    assert!(!info.overwrites_unequal);
    rig.tool.escape();

    // Unlinked drag: the others stay idle, the limit is per corner.
    let mut rig = Rig::new(radii(0.0, 40.0, 0.0, 30.0));
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    let from = rig.knob(Corner::Tl);
    rig.press(from, false);
    assert_eq!(rig.tool.corner_drag_changes_all(), Some(false));
    let info = rig
        .tool
        .live_param_drag(along(from, Corner::Tl, 500.0))
        .unwrap();
    assert!(info.limited && !info.overwrites_unequal);
    rig.tool.escape();
    assert_eq!(rig.tool.corner_drag_changes_all(), None);
}

/// Criterion 3 (last sentence): an unlinked drag on a shrunk rectangle writes
/// the three others at their effective values, so nothing visible moves.
#[test]
fn an_unlinked_drag_on_a_shrunk_rectangle_freezes_the_other_corners_as_drawn() {
    // 100 x 60: TL 50, TR 50 -> f = 100/100 = 1 ... use BL too: TL + BL = 100 > 60.
    let mut rig = Rig::new(radii(50.0, 50.0, 0.0, 50.0));
    let drawn_before = effective(&rig);
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    rig.drag(Corner::Br, false, 6.0);
    let after = rig.stored();
    assert!(after.br.as_mm() > 0.0);
    for corner in [Corner::Tl, Corner::Tr, Corner::Bl] {
        assert!(
            close(after.get(corner).as_mm(), drawn_before.get(corner).as_mm()),
            "{corner:?}: stored {after:?} vs drawn {drawn_before:?}"
        );
    }
    // Nothing drawn moved.
    let drawn_after = effective(&rig);
    for corner in [Corner::Tl, Corner::Tr, Corner::Bl] {
        assert!(close(
            drawn_after.get(corner).as_mm(),
            drawn_before.get(corner).as_mm()
        ));
    }
}

fn effective(rig: &Rig) -> CornerRadii {
    let ObjectSnapshot::Primitive(PrimitiveSnapshot {
        shape: Shape::Rect {
            bounds,
            corner_radii,
        },
        ..
    }) = rig.objects().remove(0)
    else {
        panic!("a rectangle");
    };
    curvyo_document_core::effective_corner_radii(bounds, corner_radii)
}

/// Opens the typed entry on `corner`'s knob with a first press and a
/// double-click carrying `shift` (the second press's modifiers).
fn open_entry(rig: &mut Rig, corner: Corner, shift: bool) {
    let at = rig.knob(corner);
    rig.press(at, false);
    rig.release(at);
    let outcome = rig.tool.double_click(
        &rig.objects(),
        &rig.selection,
        at,
        SEGMENT_TOLERANCE,
        tolerances(),
        (shift, false),
    );
    assert_eq!(outcome, SelectDoubleClickOutcome::EntryOpened);
}

/// Criterion 6: the scope is the switch and Shift at the second press, fixed
/// when the field opens; Enter writes all four or that corner.
#[test]
fn the_typed_entry_scope_is_fixed_at_the_second_press() {
    for (linking, shift, one_corner) in [
        (CornerLinking::Linked, false, false),
        (CornerLinking::Linked, true, true),
        (CornerLinking::Unlinked, false, true),
        (CornerLinking::Unlinked, true, false),
    ] {
        let mut rig = Rig::new(radii(5.0, 8.0, 12.0, 3.0));
        rig.tool.set_corner_linking(linking);
        open_entry(&mut rig, Corner::Tr, shift);
        let entry = rig.tool.param_entry().expect("an entry is open");
        assert_eq!(
            entry.scope(),
            Some(if one_corner {
                "This corner only"
            } else {
                "All four corners"
            })
        );
        let outcome = rig.tool.commit_entry(&rig.document, ["20", ""], 0);
        assert_eq!(outcome, EntryOutcome::Committed);
        let after = rig.stored();
        if one_corner {
            assert_eq!(after, radii(5.0, 20.0, 12.0, 3.0));
        } else {
            assert_eq!(after, CornerRadii::uniform(Length::from_mm(20.0)));
        }
    }
}

/// Criterion 8: a click on the switch closes an open entry without writing.
#[test]
fn clicking_the_switch_closes_an_open_entry_without_writing() {
    let mut rig = Rig::new(radii(5.0, 8.0, 12.0, 3.0));
    open_entry(&mut rig, Corner::Tl, false);
    assert!(rig.tool.param_entry().is_some());
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    assert!(rig.tool.param_entry().is_none());
    assert_eq!(rig.stored(), radii(5.0, 8.0, 12.0, 3.0));
}

/// Criterion 8: "Remove rounding" sets all four radii to 0 whatever the state
/// of the switch.
#[test]
fn remove_rounding_ignores_the_switch() {
    for linking in [CornerLinking::Linked, CornerLinking::Unlinked] {
        let mut rig = Rig::new(radii(5.0, 8.0, 12.0, 3.0));
        rig.tool.set_corner_linking(linking);
        let objects = rig.objects();
        rig.tool
            .remove_rounding(&rig.document, &objects, &rig.selection);
        assert_eq!(rig.stored(), radii(0.0, 0.0, 0.0, 0.0));
    }
}

/// The bar's Radius field ignores the switch: it sets all four (criterion 22).
#[test]
fn the_bar_radius_field_sets_all_four_whatever_the_switch() {
    let mut rig = Rig::new(radii(5.0, 8.0, 12.0, 3.0));
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    let objects = rig.objects();
    let outcome = rig
        .tool
        .commit_bar_radius_text(&rig.document, &objects, &rig.selection, "7");
    assert_eq!(outcome, EntryOutcome::Committed);
    assert_eq!(rig.stored(), CornerRadii::uniform(Length::from_mm(7.0)));
}
