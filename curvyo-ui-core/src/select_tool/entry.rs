//! The Select tool's numeric-entry and double-click dispatch
//! (`specs/0008-object-transform-refinements/specification.md`, criteria 3, 18,
//! 22, 23, 25-32, 49): opening, committing and cancelling the typed entry,
//! and routing a double-click to a handle, the handoff or nothing. Split
//! out of `select_tool.rs`; a child module, so it shares `SelectTool`'s
//! private state.

use curvyo_document_core::{Document, ObjectSnapshot, Point, Tolerance};

use super::{SelectDoubleClickOutcome, SelectDrag, SelectTool, sole_selected};
use crate::ResizeDirection;
use crate::anchor_id_minter::AnchorIdMinter;
use crate::group_entry::GroupEntry;
use crate::hit_test_object::hit_test_object;
use crate::move_entry::MoveEntry;
use crate::object_selection::ObjectSelection;
use crate::oriented_box::oriented_bounds;
use crate::param_entry::ParamEntry;
use crate::skew_entry::SkewEntry;
use crate::transform_entry::{EntryOutcome, TransformEntry};
use crate::transform_handle_layout::{EditHandle, Side, TransformHandleTolerances};

/// The one open numeric entry: a transform entry (angle, size, outer radius),
/// a parameter entry (corner radius, inner ratio), a skew entry or a move
/// entry. Four concrete types use the enum, each with its own fields and
/// commit.
#[derive(Debug, Clone)]
pub(super) enum OpenEntry {
    /// An angle, size or outer-radius entry.
    Transform(TransformEntry),
    /// A corner-radius or inner-ratio entry.
    Param(ParamEntry),
    /// A path's skew angle entry.
    Skew(SkewEntry),
    /// A typed move (relative or absolute).
    Move(MoveEntry),
    /// An angle, size or skew of a multi-selection.
    Group(GroupEntry),
}

/// How the move chip reads its two fields at Enter: its mode switch and its
/// Copy check, both held by the DOM and passed with the commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveEntryMode {
    /// Absolute position of the top-left of the drawn bounds, not a relative
    /// offset.
    pub absolute: bool,
    /// Copy the object instead of moving it.
    pub copy: bool,
}

/// The typed entry a key opens (`specs/0010-edit-interaction-polish/` criteria
/// 54, 57): the same entry a double-click on the matching handle opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKey {
    /// R: the angle entry of the top-right corner rotate handle.
    Angle,
    /// S: the size entry of the bottom-right corner resize handle.
    Size,
    /// M: the typed move of the centre handle.
    Move,
    /// K: the skew-x entry of the top skew handle (a path).
    SkewX,
    /// Shift+K: the skew-y entry of the right skew handle (a path).
    SkewY,
}

/// Why an entry key could not act (criterion 59): the text of the hint chip
/// is the frontend's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEntryRefusal {
    /// Nothing is selected.
    NothingSelected,
    /// K or Shift+K with a selection that holds an object that is not a path.
    SkewNeedsPath,
}

impl OpenEntry {
    pub(super) fn handle(&self) -> EditHandle {
        match self {
            Self::Transform(entry) => entry.handle(),
            Self::Param(entry) => entry.handle(),
            Self::Skew(entry) => entry.handle(),
            Self::Group(entry) => entry.handle(),
            Self::Move(_) => EditHandle::Move,
        }
    }
}

impl SelectTool {
    /// The open angle, size or outer-radius entry, if any.
    #[must_use]
    pub const fn entry(&self) -> Option<&TransformEntry> {
        match &self.entry {
            Some(OpenEntry::Transform(entry)) => Some(entry),
            _ => None,
        }
    }

    /// The open corner-radius or inner-ratio entry, if any.
    #[must_use]
    pub const fn param_entry(&self) -> Option<&ParamEntry> {
        match &self.entry {
            Some(OpenEntry::Param(entry)) => Some(entry),
            _ => None,
        }
    }

    /// The open skew-angle entry, if any.
    #[must_use]
    pub const fn skew_entry(&self) -> Option<&SkewEntry> {
        match &self.entry {
            Some(OpenEntry::Skew(entry)) => Some(entry),
            _ => None,
        }
    }

    /// The open typed move, if any.
    #[must_use]
    pub const fn move_entry(&self) -> Option<&MoveEntry> {
        match &self.entry {
            Some(OpenEntry::Move(entry)) => Some(entry),
            _ => None,
        }
    }

    /// Whether any numeric entry is open.
    #[must_use]
    pub const fn has_entry(&self) -> bool {
        self.entry.is_some()
    }

    /// The handle the open entry belongs to: it keeps its dragging look while
    /// the chip is open.
    #[must_use]
    pub fn entry_handle(&self) -> Option<EditHandle> {
        if self.centre_chip_open() {
            return None;
        }
        self.entry.as_ref().map(OpenEntry::handle)
    }

    /// Whether the open entry is the size chip of the key S: it sits by the
    /// box centre, the centre handle makes way for the pivot marker, and no
    /// handle is highlighted (`edit-interaction-polish` criterion 59).
    #[must_use]
    pub fn centre_chip_open(&self) -> bool {
        match &self.entry {
            Some(OpenEntry::Transform(entry)) => entry.centre_chip(),
            Some(OpenEntry::Group(entry)) => entry.centre_chip(),
            _ => false,
        }
    }

    /// Closes the numeric entry without writing (criterion 20): idempotent.
    pub fn cancel_entry(&mut self) {
        self.entry = None;
    }

    /// Validates and commits the numeric entry (criteria 19, 21, 27, 30,
    /// 31): `texts` are the field texts, `last_edited` the index of the
    /// field the maker edited last. A committed or unchanged entry closes;
    /// an invalid one stays open. An entry whose object changed or went
    /// away since it opened is dropped without writing.
    pub fn commit_entry(
        &mut self,
        document: &Document,
        texts: [&str; 2],
        last_edited: usize,
    ) -> EntryOutcome {
        let Some(entry) = self.entry.as_ref() else {
            return EntryOutcome::Unchanged;
        };
        let outcome = match entry {
            OpenEntry::Transform(entry) => entry.commit(document, texts, last_edited),
            OpenEntry::Param(entry) => entry.commit(document, texts[0]),
            OpenEntry::Skew(entry) => entry.commit(document, texts[0]),
            OpenEntry::Group(entry) => entry.commit(document, texts, last_edited),
            // The typed move has its own commit: it needs the mode.
            OpenEntry::Move(_) => EntryOutcome::Unchanged,
        };
        if !matches!(outcome, EntryOutcome::Invalid { .. }) {
            self.entry = None;
        }
        outcome
    }

    /// Enter in the move chip (criteria 20, 21, 22, 23, 25): `texts` are the X
    /// and Y texts, `absolute` the chip's mode and `copy` the state of its Copy
    /// check. A committed or unchanged entry closes; an invalid one stays
    /// open. After a typed copy the selection is the copy (criterion 35,
    /// default of flag 5).
    pub fn commit_move_entry(
        &mut self,
        document: &Document,
        selection: &mut ObjectSelection,
        minter: &mut AnchorIdMinter,
        texts: [&str; 2],
        mode: MoveEntryMode,
    ) -> EntryOutcome {
        let Some(OpenEntry::Move(entry)) = self.entry.as_ref() else {
            return EntryOutcome::Unchanged;
        };
        let (outcome, copies) = entry.commit(document, texts, mode.absolute, mode.copy, minter);
        if !matches!(outcome, EntryOutcome::Invalid { .. }) {
            self.entry = None;
        }
        if let Some(copies) = copies {
            selection.set(&copies);
        }
        outcome
    }

    /// Criteria 56 to 58: opens the typed move (M), the angle entry (R), the
    /// size entry (S) or the skew entry (K, Shift+K) for the sole selected
    /// object, as a double-click on the centre handle, the top-right corner
    /// rotate handle, the bottom-right corner resize handle, the top or the
    /// right skew handle does, with no Shift pivot and no Ctrl link: those
    /// are read only from a double-click. One more difference, set by the
    /// customer on 2026-10-07 (criterion 57a): S scales about the box centre,
    /// as a Shift drag would, because no handle was chosen. The entries do not
    /// depend on the handle being drawn (`entry_anchor` places the chip), so
    /// they open for an object of any size.
    ///
    /// # Errors
    /// [`KeyEntryRefusal`] when the selection is not exactly one object, or
    /// when K or Shift+K meets a selected object that is not a path; nothing
    /// changes then.
    pub fn open_entry_for_key(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        key: EntryKey,
    ) -> Result<(), KeyEntryRefusal> {
        let object = match selection.ids() {
            [] => return Err(KeyEntryRefusal::NothingSelected),
            [_] => sole_selected(objects, selection).ok_or(KeyEntryRefusal::NothingSelected)?,
            _ => {
                let group =
                    Self::group_of(objects, selection).ok_or(KeyEntryRefusal::NothingSelected)?;
                return self.open_group_entry_for_key(objects, selection, &group, key);
            }
        };
        let box_ = oriented_bounds(object);
        let skew = |side| {
            SkewEntry::for_handle(object, &box_, side, false)
                .map(OpenEntry::Skew)
                .ok_or(KeyEntryRefusal::SkewNeedsPath)
        };
        let entry = match key {
            EntryKey::Angle => OpenEntry::Transform(TransformEntry::for_rotate(
                object,
                &box_,
                ResizeDirection::Ne,
                false,
            )),
            // No handle was chosen: the typed size scales about the box centre,
            // as a drag with Shift held would (criterion 57a), and reads no
            // modifier (Ctrl+S is gated, so no Ctrl link either).
            EntryKey::Size => OpenEntry::Transform(
                TransformEntry::for_resize(
                    object,
                    &box_,
                    ResizeDirection::Se,
                    (true, false),
                    self.modes,
                )
                .with_centre_chip(),
            ),
            EntryKey::Move => OpenEntry::Move(MoveEntry::new(object, &box_)),
            EntryKey::SkewX => skew(Side::Top)?,
            EntryKey::SkewY => skew(Side::Right)?,
        };
        self.entry = Some(entry);
        Ok(())
    }

    /// Acceptance criteria 3, 15, 16, 18, 22, 23, 25-28, 32: the double-click
    /// dispatch. A double-click on a handle (at the *second press's*
    /// position and modifiers, and only if the first press grabbed the same
    /// handle) opens the numeric entry for a rotate, resize, skew or
    /// parameter handle, and the typed move for the centre handle, where it
    /// is drawn: no handoff to the object's own tool. Anywhere else inside the
    /// box of the sole selected object, the centre handle's region excluded,
    /// hands off to it; outside, slice 4's outline hit decides.
    pub fn double_click(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerance: Tolerance,
        handle_tolerances: TransformHandleTolerances,
        modifiers: (bool, bool),
    ) -> SelectDoubleClickOutcome {
        let (shift, ctrl) = modifiers;
        self.drag = SelectDrag::None;
        // A multi-selection: its group handles (criterion 47); anywhere else the
        // double-click is the one of an object.
        if selection.ids().len() >= 2 {
            if let Some(outcome) =
                self.group_double_click(objects, selection, point, handle_tolerances, modifiers)
            {
                return outcome;
            }
            return double_click(objects, point, tolerance);
        }
        // Kept (not consumed): a rapid third press is another double-click on
        // the same handle and keeps its entry.
        let first_press_handle = self.last_press_handle;
        // The centre handle is hit last, only inside its hover region, so it
        // never wins against another handle (criterion 16).
        let hit = Self::hover_handle_at(objects, selection, point, handle_tolerances, shift)
            .filter(|(_, _, handle)| first_press_handle == Some(*handle));
        if let Some((object, box_, handle)) = hit {
            let entry = match handle {
                EditHandle::Rotate(direction) => Some(OpenEntry::Transform(
                    TransformEntry::for_rotate(object, &box_, direction, shift),
                )),
                EditHandle::Resize(direction) => Some(OpenEntry::Transform(
                    TransformEntry::for_resize(object, &box_, direction, (shift, ctrl), self.modes),
                )),
                EditHandle::Param(param) => ParamEntry::for_handle(
                    object,
                    &box_,
                    param,
                    self.corner_linking.is_unlinked_with(shift),
                )
                .map(OpenEntry::Param),
                EditHandle::Skew(side) => {
                    SkewEntry::for_handle(object, &box_, side, shift).map(OpenEntry::Skew)
                }
                EditHandle::Move => Some(OpenEntry::Move(
                    MoveEntry::new(object, &box_).with_copy_preset(ctrl),
                )),
            };
            return match entry {
                Some(entry) => {
                    self.entry = Some(entry);
                    SelectDoubleClickOutcome::EntryOpened
                }
                None => SelectDoubleClickOutcome::Miss,
            };
        }
        if Self::is_inside_selected_box(objects, selection, point)
            && let Some(object) = sole_selected(objects, selection)
        {
            return hit_outcome(object);
        }
        double_click(objects, point, tolerance)
    }
}

/// What a double-click on an object's outline hit: a path for `Session` to
/// hand off to the Node tool (criterion 22 of slice 4), or the edit hint for a
/// primitive (criterion 32 of `unified-object-editing`).
#[must_use]
pub fn double_click(
    objects: &[ObjectSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> SelectDoubleClickOutcome {
    let Some(hit) = hit_test_object(objects, point, tolerance) else {
        return SelectDoubleClickOutcome::Miss;
    };
    objects
        .iter()
        .find(|object| object.id() == hit)
        .map_or(SelectDoubleClickOutcome::Miss, hit_outcome)
}

/// A path is handed off to the Node tool; a primitive only earns the edit
/// hint (criteria 31, 32); a compound path has no editable nodes yet, so it
/// stays in the Select tool (`specs/0016-boolean-operations` criterion 38).
fn hit_outcome(object: &ObjectSnapshot) -> SelectDoubleClickOutcome {
    match object {
        ObjectSnapshot::Path(path) if path.is_compound() => SelectDoubleClickOutcome::CompoundPath,
        ObjectSnapshot::Path(_) => SelectDoubleClickOutcome::Hit(object.clone()),
        ObjectSnapshot::Primitive(_) => SelectDoubleClickOutcome::EditHint,
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{
        AnchorId, Angle, Document, InnerRatio, Length, NewAnchor, PointCount, RectBounds, Shape,
        StarFrame,
    };

    use super::*;
    use crate::transform_drag::ScaleModes;
    use crate::transform_entry::EntryKind;

    fn rect_at(document: &Document, x: f64, size: f64) -> curvyo_document_core::NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(x, 0.0),
            width: Length::from_mm(size),
            height: Length::from_mm(size / 2.0),
        })
    }

    fn open(
        document: &Document,
        ids: &[curvyo_document_core::NodeId],
        key: EntryKey,
    ) -> (SelectTool, Result<(), KeyEntryRefusal>) {
        let objects: Vec<ObjectSnapshot> = document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect();
        let mut selection = ObjectSelection::new();
        for id in ids {
            selection.toggle(*id);
        }
        let mut tool = SelectTool::new();
        let result = tool.open_entry_for_key(&objects, &selection, key);
        (tool, result)
    }

    /// Criterion 57: R opens the angle entry of the top-right corner rotate
    /// handle, S the size entry of the bottom-right corner resize handle, for
    /// any object and any size, with the box centre as pivot.
    #[test]
    fn r_and_s_open_the_entries_of_the_two_corner_handles() {
        let document = Document::new(1);
        for size in [0.5, 4.0, 400.0] {
            let id = rect_at(&document, 0.0, size);
            let (tool, result) = open(&document, &[id], EntryKey::Angle);
            assert_eq!(result, Ok(()));
            let entry = tool.entry().expect("an entry");
            assert_eq!(entry.kind(), EntryKind::Angle);
            assert_eq!(entry.handle(), EditHandle::Rotate(ResizeDirection::Ne));
            assert_eq!(
                tool.entry_handle(),
                Some(EditHandle::Rotate(ResizeDirection::Ne))
            );
            let (tool, result) = open(&document, &[id], EntryKey::Size);
            assert_eq!(result, Ok(()));
            let entry = tool.entry().expect("an entry");
            assert_eq!(entry.kind(), EntryKind::Size);
            assert_eq!(entry.handle(), EditHandle::Resize(ResizeDirection::Se));
            // The key opens its chip by the centre and highlights no handle.
            assert!(entry.centre_chip() && tool.centre_chip_open());
            assert_eq!(tool.entry_handle(), None);
            assert_eq!(entry.fields().len(), 2, "width and height");
            assert!(tool.has_entry());
        }
    }

    /// The size entry of a polygon or star is the outer radius, as for the
    /// double-click route.
    #[test]
    fn s_on_a_polygon_or_star_opens_the_outer_radius() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(12.0),
            angle: Angle::from_radians(0.0),
        };
        let polygon = document.create_polygon(frame, PointCount::new(5).expect("count"));
        let star = document.create_star(
            frame,
            PointCount::new(5).expect("count"),
            InnerRatio::new(0.5).expect("ratio"),
        );
        for id in [polygon, star] {
            let (tool, result) = open(&document, &[id], EntryKey::Size);
            assert_eq!(result, Ok(()));
            let entry = tool.entry().expect("an entry");
            assert_eq!(entry.kind(), EntryKind::OuterRadius);
            assert_eq!(entry.fields()[0].prefill, "12.0");
        }
    }

    /// Criterion 59: nothing selected refuses and opens nothing; several selected
    /// open the entry of the group box (`multi-object-transform` criteria 33, 34).
    #[test]
    fn nothing_selected_refuses_and_several_open_the_group_entry() {
        let document = Document::new(1);
        let a = rect_at(&document, 0.0, 10.0);
        let b = rect_at(&document, 20.0, 10.0);
        for key in [EntryKey::Angle, EntryKey::Size] {
            let (tool, result) = open(&document, &[], key);
            assert_eq!(result, Err(KeyEntryRefusal::NothingSelected));
            assert!(!tool.has_entry());
            let (tool, result) = open(&document, &[a, b], key);
            assert_eq!(result, Ok(()));
            assert!(tool.has_entry());
            assert!(tool.group_entry().is_some());
        }
    }

    fn rect_of(document: &Document, id: curvyo_document_core::NodeId) -> (Point, Point) {
        let ObjectSnapshot::Primitive(primitive) = document.object(id).expect("exists") else {
            panic!("a primitive");
        };
        let Shape::Rect { bounds, .. } = primitive.shape else {
            panic!("a rectangle");
        };
        (
            bounds.origin,
            Point::new(
                bounds.origin.x + bounds.width.as_mm(),
                bounds.origin.y + bounds.height.as_mm(),
            ),
        )
    }

    /// Criteria 57, 57a (customer, 2026-10-07): the key S scales about the box
    /// centre, never about the corner opposite the bottom-right handle: a 40 x
    /// 20 mm rectangle at (10, 10) typed to 60 x 30 ends at (0, 5)..(60, 35).
    #[test]
    fn s_scales_about_the_box_centre() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(10.0, 10.0),
            width: Length::from_mm(40.0),
            height: Length::from_mm(20.0),
        });
        let (mut tool, result) = open(&document, &[id], EntryKey::Size);
        assert_eq!(result, Ok(()));
        let pivot = tool.entry().expect("an entry").pivot();
        assert!((pivot.x - 30.0).abs() < 1e-9 && (pivot.y - 20.0).abs() < 1e-9);
        assert_eq!(
            tool.commit_entry(&document, ["60", "30"], 1),
            EntryOutcome::Committed
        );
        let (min, max) = rect_of(&document, id);
        for (got, want) in [(min.x, 0.0), (min.y, 5.0), (max.x, 60.0), (max.y, 35.0)] {
            assert!((got - want).abs() < 1e-9, "{got} vs {want}");
        }
    }

    /// Criterion 57a: the S route and a double-click on the same handle differ
    /// only in the fixed point; the double-click keeps the opposite corner, and
    /// with Shift at the second press the centre (so S equals that Shift route).
    #[test]
    fn the_double_click_route_keeps_the_dragged_handles_fixed_point() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(10.0, 10.0),
            width: Length::from_mm(40.0),
            height: Length::from_mm(20.0),
        });
        let object = document.object(id).expect("exists");
        let box_ = oriented_bounds(&object);
        let plain = TransformEntry::for_resize(
            &object,
            &box_,
            ResizeDirection::Se,
            (false, false),
            ScaleModes::default(),
        );
        assert_eq!(plain.pivot(), Point::new(10.0, 10.0), "opposite corner");
        assert!(
            !plain.centre_chip(),
            "the double-click chip sits at the handle"
        );
        let by_key = {
            let (tool, _) = open(&document, &[id], EntryKey::Size);
            tool.entry().expect("an entry").clone()
        };
        let shifted = TransformEntry::for_resize(
            &object,
            &box_,
            ResizeDirection::Se,
            (true, false),
            ScaleModes::default(),
        );
        assert_eq!(by_key.pivot(), shifted.pivot());
        assert!(
            !by_key.linked(),
            "Ctrl+S is gated: the fields are independent"
        );
        let corner = plain
            .resolve(["60", "30"], 1)
            .expect("valid")
            .expect("changes");
        let centre = by_key
            .resolve(["60", "30"], 1)
            .expect("valid")
            .expect("changes");
        assert_ne!(corner, centre);
    }

    /// A path opens the same entries.
    #[test]
    fn a_path_opens_an_angle_entry() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 5.0)),
            ],
            false,
        );
        let (tool, result) = open(&document, &[id], EntryKey::Angle);
        assert_eq!(result, Ok(()));
        assert_eq!(tool.entry().expect("an entry").kind(), EntryKind::Angle);
    }
}
