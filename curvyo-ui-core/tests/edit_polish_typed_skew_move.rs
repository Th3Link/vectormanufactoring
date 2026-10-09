//! `curvyo-ui-core`'s share of Part B of
//! `specs/0010-edit-interaction-polish/specification.md`, PR 3: the double-click
//! on the centre handle and on a skew handle, the keys M, K and Shift+K, and
//! the entries they open. Session-level behaviour (commit counts, hints,
//! save and reopen) is in `curvyo-editor-wasm/tests/edit_polish_typed_skew_move.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot,
    Point, PointCount, RectBounds, StarFrame, Tolerance, Vec2,
};
use curvyo_ui_core::{
    AnchorIdMinter, EditHandle, EntryKey, EntryKind, EntryOutcome, InvalidReason, KeyEntryRefusal,
    Modifiers, MoveEntryMode, ObjectSelection, SelectDoubleClickOutcome, SelectTool, Side,
    TransformHandleTolerances, object_outline_bounds, oriented_bounds,
};

const SEGMENT_TOLERANCE: Tolerance = Tolerance::from_mm(1.0);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Two screen pixels per millimetre: the centre handle needs a 24 mm shorter
/// side, its hover region has the radius `min(12 px, s / 4)`.
fn tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(2.0)
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Rect,
    Ellipse,
    Polygon,
    Star,
    Path,
}

const ALL_KINDS: [Kind; 5] = [
    Kind::Rect,
    Kind::Ellipse,
    Kind::Polygon,
    Kind::Star,
    Kind::Path,
];

struct Rig {
    document: Document,
    id: NodeId,
    selection: ObjectSelection,
    tool: SelectTool,
}

impl Rig {
    /// An object of `kind` about `size` mm wide and high, selected.
    fn new(kind: Kind, size: f64) -> Self {
        let document = Document::new(1);
        let c = pt(60.0, 50.0);
        let id = match kind {
            Kind::Rect => document.create_rect(RectBounds {
                origin: pt(c.x - size / 2.0, c.y - size / 3.0),
                width: Length::from_mm(size),
                height: Length::from_mm(size * 2.0 / 3.0),
            }),
            Kind::Ellipse => document.create_ellipse(EllipseFrame {
                center: c,
                rx: Length::from_mm(size / 2.0),
                ry: Length::from_mm(size / 3.0),
            }),
            Kind::Polygon => document.create_polygon(
                StarFrame {
                    center: c,
                    radius: Length::from_mm(size / 2.0),
                    angle: Angle::from_radians(0.0),
                },
                PointCount::new(6).unwrap(),
            ),
            Kind::Star => document.create_star(
                StarFrame {
                    center: c,
                    radius: Length::from_mm(size / 2.0),
                    angle: Angle::from_radians(0.0),
                },
                PointCount::new(5).unwrap(),
                InnerRatio::new(0.5).unwrap(),
            ),
            Kind::Path => document.create_path(
                &[
                    NewAnchor::corner(AnchorId::new(1, 1), pt(c.x - size / 2.0, c.y - size / 3.0)),
                    NewAnchor::corner(AnchorId::new(1, 2), pt(c.x + size / 2.0, c.y - size / 3.0)),
                    NewAnchor::corner(AnchorId::new(1, 3), pt(c.x + size / 2.0, c.y + size / 3.0)),
                    NewAnchor::corner(AnchorId::new(1, 4), pt(c.x - size / 2.0, c.y + size / 3.0)),
                ],
                true,
            ),
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

    fn centre(&self) -> Point {
        let b = oriented_bounds(&self.object());
        b.to_document(b.local_center())
    }

    fn handle(&self, wanted: EditHandle) -> Point {
        SelectTool::transform_handles(&self.objects(), &self.selection, tolerances(), false)
            .into_iter()
            .find(|(h, _)| *h == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} is drawn"))
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
            Modifiers::new(false, false),
            &mut AnchorIdMinter::new(99),
        );
    }

    fn second_press(&mut self, at: Point, modifiers: (bool, bool)) -> SelectDoubleClickOutcome {
        let objects = self.objects();
        self.tool.double_click(
            &objects,
            &self.selection,
            at,
            SEGMENT_TOLERANCE,
            tolerances(),
            modifiers,
        )
    }

    fn double_click(&mut self, at: Point) -> SelectDoubleClickOutcome {
        self.press(at, false);
        self.release(at);
        self.second_press(at, (false, false))
    }

    fn key(&mut self, key: EntryKey) -> Result<(), KeyEntryRefusal> {
        let objects = self.objects();
        self.tool.open_entry_for_key(&objects, &self.selection, key)
    }
}

/// Criteria 15, 16: a double-click on the drawn centre handle opens the typed
/// move for a path and for every primitive; nothing is written, nothing else
/// changes.
#[test]
fn a_double_click_on_the_centre_handle_opens_the_typed_move() {
    for kind in ALL_KINDS {
        let mut rig = Rig::new(kind, 100.0);
        let before = rig.object();
        let centre = rig.centre();
        assert_eq!(
            rig.double_click(centre),
            SelectDoubleClickOutcome::EntryOpened,
            "{kind:?}"
        );
        let entry = rig.tool.move_entry().expect("a move entry");
        assert_eq!(entry.fields().len(), 2);
        assert_eq!(entry.fields()[0].prefill, "0");
        assert_eq!(rig.tool.entry_handle(), Some(EditHandle::Move));
        assert!(rig.tool.entry().is_none() && rig.tool.skew_entry().is_none());
        assert_eq!(rig.object(), before, "{kind:?}: the presses write nothing");
        assert_eq!(rig.selection.ids(), &[rig.id]);
    }
}

/// Criterion 16: inside the box but outside the centre handle's region the
/// old rule holds (path: handoff; primitive: the edit hint), and so on the
/// outline.
#[test]
fn a_double_click_inside_the_box_outside_the_centre_region_keeps_the_old_rule() {
    for kind in ALL_KINDS {
        let mut rig = Rig::new(kind, 100.0);
        let before = rig.object();
        // 20 px at 2 px per mm is well outside the 12 px hover radius.
        let away = rig.centre().translated(Vec2::new(10.0, 5.0));
        let outcome = rig.double_click(away);
        let expected = if matches!(kind, Kind::Path) {
            SelectDoubleClickOutcome::Hit(before.clone())
        } else {
            SelectDoubleClickOutcome::EditHint
        };
        assert_eq!(outcome, expected, "{kind:?}");
        assert!(rig.tool.move_entry().is_none());
    }
}

/// Criterion 17: where the centre handle is not drawn (a box under 48 px)
/// a double-click keeps the old rule and opens no typed move; the key M opens
/// it all the same.
#[test]
fn where_the_centre_handle_is_not_drawn_the_double_click_opens_no_move_but_m_does() {
    for kind in ALL_KINDS {
        let mut rig = Rig::new(kind, 15.0);
        let handles =
            SelectTool::transform_handles(&rig.objects(), &rig.selection, tolerances(), false);
        assert!(
            handles.iter().all(|(h, _)| *h != EditHandle::Move),
            "{kind:?}"
        );
        let centre = rig.centre();
        let outcome = rig.double_click(centre);
        assert!(
            matches!(
                outcome,
                SelectDoubleClickOutcome::Hit(_) | SelectDoubleClickOutcome::EditHint
            ),
            "{kind:?}: {outcome:?}"
        );
        assert!(rig.tool.move_entry().is_none());
        assert_eq!(rig.key(EntryKey::Move), Ok(()), "{kind:?}");
        assert!(rig.tool.move_entry().is_some());
    }
}

/// Criterion 15: the first press must have been on the centre handle too: a
/// first press elsewhere in the box and a second on the centre opens nothing.
#[test]
fn the_first_press_must_have_grabbed_the_centre_handle() {
    let mut rig = Rig::new(Kind::Rect, 100.0);
    let away = rig.centre().translated(Vec2::new(10.0, 5.0));
    rig.press(away, false);
    rig.release(away);
    let outcome = rig.second_press(rig.centre(), (false, false));
    assert!(!matches!(outcome, SelectDoubleClickOutcome::EntryOpened));
    assert!(rig.tool.move_entry().is_none());
}

/// Criterion 16: a press on the centre handle is still a move press.
#[test]
fn a_drag_from_the_centre_handle_still_moves() {
    let mut rig = Rig::new(Kind::Rect, 100.0);
    let (before, _) = object_outline_bounds(&rig.object());
    let centre = rig.centre();
    rig.press(centre, false);
    let to = centre.translated(Vec2::new(20.0, 10.0));
    rig.tool
        .pointer_moved(to, Modifiers::NONE, &mut rig.selection);
    rig.release(to);
    let (after, _) = object_outline_bounds(&rig.object());
    assert!((after.x - before.x - 20.0).abs() < 1e-9);
    assert!((after.y - before.y - 10.0).abs() < 1e-9);
}

/// Criterion 9: a double-click on a skew handle opens the skew entry, the
/// presses write nothing and there is no handoff; the pivot is the opposite
/// side's line, or the centre line with Shift at the second press.
#[test]
fn a_double_click_on_a_skew_handle_opens_the_skew_entry() {
    for side in Side::ALL {
        let mut rig = Rig::new(Kind::Path, 100.0);
        let before = rig.object();
        let at = rig.handle(EditHandle::Skew(side));
        rig.press(at, false);
        rig.release(at);
        let objects = rig.objects();
        let outcome = rig.tool.double_click(
            &objects,
            &rig.selection,
            at,
            SEGMENT_TOLERANCE,
            tolerances(),
            (false, false),
        );
        assert_eq!(outcome, SelectDoubleClickOutcome::EntryOpened, "{side:?}");
        let entry = rig.tool.skew_entry().expect("a skew entry");
        assert_eq!(entry.kind(), EntryKind::Skew);
        assert_eq!(entry.handle(), EditHandle::Skew(side));
        assert_eq!(rig.tool.entry_handle(), Some(EditHandle::Skew(side)));
        assert_eq!(rig.object(), before);
        let fixed = entry.pivot();
        let b = oriented_bounds(&before);
        let want = match side {
            Side::Top => pt(60.0, b.to_document(pt(0.0, b.max.y)).y),
            Side::Bottom => pt(60.0, b.to_document(pt(0.0, b.min.y)).y),
            Side::Left => pt(b.to_document(pt(b.max.x, 0.0)).x, 50.0),
            Side::Right => pt(b.to_document(pt(b.min.x, 0.0)).x, 50.0),
        };
        match side {
            Side::Top | Side::Bottom => assert!((fixed.y - want.y).abs() < 1e-9, "{side:?}"),
            Side::Left | Side::Right => assert!((fixed.x - want.x).abs() < 1e-9, "{side:?}"),
        }
        rig.tool.cancel_entry();
        // Shift at the second press: the centre line.
        rig.press(at, false);
        rig.release(at);
        let objects = rig.objects();
        let shifted = {
            // The Shift state reveals no side rotate handle over a skew handle.
            rig.tool.double_click(
                &objects,
                &rig.selection,
                at,
                SEGMENT_TOLERANCE,
                tolerances(),
                (true, false),
            )
        };
        assert_eq!(shifted, SelectDoubleClickOutcome::EntryOpened);
        let centre = rig.tool.skew_entry().unwrap().pivot();
        match side {
            Side::Top | Side::Bottom => assert!((centre.y - 50.0).abs() < 1e-9),
            Side::Left | Side::Right => assert!((centre.x - 60.0).abs() < 1e-9),
        }
    }
}

/// Criterion 9: a first press elsewhere and a second on the skew handle opens
/// nothing.
#[test]
fn the_first_press_must_have_grabbed_the_skew_handle() {
    let mut rig = Rig::new(Kind::Path, 100.0);
    let away = rig.centre().translated(Vec2::new(10.0, 5.0));
    rig.press(away, false);
    rig.release(away);
    let at = rig.handle(EditHandle::Skew(Side::Top));
    assert!(!matches!(
        rig.second_press(at, (false, false)),
        SelectDoubleClickOutcome::EntryOpened
    ));
    assert!(rig.tool.skew_entry().is_none());
}

/// Criteria 56, 58: M opens the typed move for every kind and size; K and
/// Shift+K open the skew entry of a path, also where the skew handle is not
/// drawn; K on anything else is refused.
#[test]
fn the_entry_keys_open_their_entries_for_any_size() {
    for size in [3.0, 15.0, 100.0] {
        for kind in ALL_KINDS {
            let mut rig = Rig::new(kind, size);
            assert_eq!(rig.key(EntryKey::Move), Ok(()), "{kind:?} {size}");
            assert!(rig.tool.move_entry().is_some());
            assert_eq!(rig.tool.entry_handle(), Some(EditHandle::Move));
            for (key, name, side) in [
                (EntryKey::SkewX, "Skew angle x", Side::Top),
                (EntryKey::SkewY, "Skew angle y", Side::Right),
            ] {
                let mut rig = Rig::new(kind, size);
                if matches!(kind, Kind::Path) {
                    assert_eq!(rig.key(key), Ok(()), "{size}");
                    let entry = rig.tool.skew_entry().unwrap();
                    assert_eq!(entry.fields()[0].accessible_name, name);
                    assert_eq!(entry.handle(), EditHandle::Skew(side));
                    // The key never uses the Shift pivot: the opposite side.
                    let b = oriented_bounds(&rig.object());
                    let fixed = entry.pivot();
                    let want = if side == Side::Top {
                        b.to_document(pt(0.0, b.max.y)).y
                    } else {
                        b.to_document(pt(b.min.x, 0.0)).x
                    };
                    assert!(
                        (if side == Side::Top { fixed.y } else { fixed.x } - want).abs() < 1e-9
                    );
                } else {
                    assert_eq!(
                        rig.key(key),
                        Err(KeyEntryRefusal::SkewNeedsPath),
                        "{kind:?}"
                    );
                    assert!(!rig.tool.has_entry());
                }
            }
        }
    }
}

/// Criterion 59: nothing selected and several selected refuse M, K and
/// Shift+K and open nothing.
#[test]
fn the_entry_keys_refuse_nothing_or_several_selected() {
    for key in [EntryKey::Move, EntryKey::SkewX, EntryKey::SkewY] {
        let mut rig = Rig::new(Kind::Path, 100.0);
        rig.selection.clear();
        assert_eq!(rig.key(key), Err(KeyEntryRefusal::NothingSelected));
        let other = rig.document.create_rect(RectBounds {
            origin: pt(300.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        rig.selection.select_single(rig.id);
        rig.selection.toggle(other);
        let objects: Vec<ObjectSnapshot> = [rig.id, other]
            .iter()
            .map(|id| rig.document.object(*id).unwrap())
            .collect();
        assert_eq!(
            rig.tool.open_entry_for_key(&objects, &rig.selection, key),
            Err(KeyEntryRefusal::SeveralSelected)
        );
        assert!(!rig.tool.has_entry());
    }
}

/// Criteria 20, 21, 25: the move entry commits through the tool: one write,
/// relative and absolute, and a no-op closes it without writing.
#[test]
fn the_move_entry_commits_through_the_tool() {
    let mut rig = Rig::new(Kind::Rect, 60.0);
    let (top_left, _) = object_outline_bounds(&rig.object());
    rig.key(EntryKey::Move).unwrap();
    assert_eq!(
        rig.tool.commit_move_entry(
            &rig.document,
            &mut rig.selection,
            &mut AnchorIdMinter::new(99),
            ["5", "-3"],
            MoveEntryMode {
                absolute: false,
                copy: false
            },
        ),
        EntryOutcome::Committed
    );
    assert!(!rig.tool.has_entry(), "a committed entry closes");
    let (moved, _) = object_outline_bounds(&rig.object());
    assert!((moved.x - top_left.x - 5.0).abs() < 1e-9);
    assert!((moved.y - top_left.y + 3.0).abs() < 1e-9);

    rig.key(EntryKey::Move).unwrap();
    assert_eq!(
        rig.tool.commit_move_entry(
            &rig.document,
            &mut rig.selection,
            &mut AnchorIdMinter::new(99),
            ["abc", "0"],
            MoveEntryMode {
                absolute: false,
                copy: false
            },
        ),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::NotANumber
        }
    );
    assert!(rig.tool.has_entry(), "an invalid entry stays open");
    assert_eq!(
        rig.tool.commit_move_entry(
            &rig.document,
            &mut rig.selection,
            &mut AnchorIdMinter::new(99),
            ["100", "50"],
            MoveEntryMode {
                absolute: true,
                copy: false
            },
        ),
        EntryOutcome::Committed
    );
    let (placed, _) = object_outline_bounds(&rig.object());
    assert!((placed.x - 100.0).abs() < 1e-9 && (placed.y - 50.0).abs() < 1e-9);

    let before = rig.object();
    rig.key(EntryKey::Move).unwrap();
    assert_eq!(
        rig.tool.commit_move_entry(
            &rig.document,
            &mut rig.selection,
            &mut AnchorIdMinter::new(99),
            ["0", "0"],
            MoveEntryMode {
                absolute: false,
                copy: false
            },
        ),
        EntryOutcome::Unchanged
    );
    assert!(!rig.tool.has_entry());
    assert_eq!(rig.object(), before);
}

/// Criterion 10: the skew entry commits through the tool and equals a skew
/// drag of the same handle ending at the same angle.
#[test]
fn the_skew_entry_commits_through_the_tool() {
    let mut rig = Rig::new(Kind::Path, 60.0);
    rig.key(EntryKey::SkewX).unwrap();
    assert_eq!(
        rig.tool.commit_entry(&rig.document, ["200", ""], 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::SkewRange
        }
    );
    assert!(rig.tool.has_entry());
    assert_eq!(
        rig.tool.commit_entry(&rig.document, ["45", ""], 0),
        EntryOutcome::Committed
    );
    assert!(!rig.tool.has_entry());
    // A path 60 wide and 40 high: the top edge moves +40 mm in x at 45 degrees.
    let ObjectSnapshot::Path(path) = rig.object() else {
        panic!("a path");
    };
    let top: Vec<f64> = path.anchors.iter().map(|a| a.point.x).collect();
    assert!((top[0] - (30.0 + 40.0)).abs() < 1e-9, "{top:?}");
    assert!(
        (top[3] - 30.0).abs() < 1e-9,
        "the bottom edge stays: {top:?}"
    );
}
