//! The Select tool's numeric-entry and double-click dispatch
//! (`specs/object-transform-refinements/specification.md`, criteria 3, 18,
//! 22, 23, 25-32, 49): opening, committing and cancelling the typed entry,
//! and routing a double-click to a handle, the handoff or nothing. Split
//! out of `select_tool.rs`; a child module, so it shares `SelectTool`'s
//! private state.

use vecmanf_document_core::{Document, ObjectSnapshot, Point, Tolerance};

use super::{SelectDoubleClickOutcome, SelectDrag, SelectTool, sole_selected};
use crate::ResizeDirection;
use crate::hit_test_object::hit_test_object;
use crate::object_selection::ObjectSelection;
use crate::oriented_box::oriented_bounds;
use crate::param_entry::ParamEntry;
use crate::transform_entry::{EntryOutcome, TransformEntry};
use crate::transform_handle_layout::{EditHandle, TransformHandleTolerances};

/// The one open numeric entry: a transform entry (angle, size, outer radius)
/// or a parameter entry (corner radius, inner ratio). Two concrete types use
/// the enum.
#[derive(Debug, Clone)]
pub(super) enum OpenEntry {
    /// An angle, size or outer-radius entry.
    Transform(TransformEntry),
    /// A corner-radius or inner-ratio entry.
    Param(ParamEntry),
}

/// The typed entry a key opens (`specs/edit-interaction-polish/` criteria
/// 54, 57): the same entry a double-click on the matching handle opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKey {
    /// R: the angle entry of the top-right corner rotate handle.
    Angle,
    /// S: the size entry of the bottom-right corner resize handle.
    Size,
}

/// Why an entry key could not act (criterion 59): the text of the hint chip
/// is the frontend's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEntryRefusal {
    /// Nothing is selected.
    NothingSelected,
    /// Several objects are selected; a typed value needs exactly one.
    SeveralSelected,
}

impl OpenEntry {
    pub(super) fn handle(&self) -> EditHandle {
        match self {
            Self::Transform(entry) => entry.handle(),
            Self::Param(entry) => entry.handle(),
        }
    }
}

impl SelectTool {
    /// The open angle, size or outer-radius entry, if any.
    #[must_use]
    pub const fn entry(&self) -> Option<&TransformEntry> {
        match &self.entry {
            Some(OpenEntry::Transform(entry)) => Some(entry),
            Some(OpenEntry::Param(_)) | None => None,
        }
    }

    /// The open corner-radius or inner-ratio entry, if any.
    #[must_use]
    pub const fn param_entry(&self) -> Option<&ParamEntry> {
        match &self.entry {
            Some(OpenEntry::Param(entry)) => Some(entry),
            Some(OpenEntry::Transform(_)) | None => None,
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
        self.entry.as_ref().map(OpenEntry::handle)
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
        };
        if !matches!(outcome, EntryOutcome::Invalid { .. }) {
            self.entry = None;
        }
        outcome
    }

    /// Criterion 57: opens the angle entry (R) or the size entry (S) for the
    /// sole selected object, as a double-click on its top-right corner rotate
    /// handle or bottom-right corner resize handle does, with no Shift pivot
    /// and no Ctrl link: those are read only from a double-click. One
    /// difference, set by the customer on 2026-10-07 (criterion 57a): S
    /// scales about the box centre, as a Shift drag would, because no handle
    /// was chosen. Both corner handles always exist, whatever the box size,
    /// so the entry opens for an object of any size.
    ///
    /// # Errors
    /// [`KeyEntryRefusal`] when the selection is not exactly one object;
    /// nothing changes then.
    pub fn open_entry_for_key(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        key: EntryKey,
    ) -> Result<(), KeyEntryRefusal> {
        let object = match selection.ids() {
            [] => return Err(KeyEntryRefusal::NothingSelected),
            [_] => sole_selected(objects, selection).ok_or(KeyEntryRefusal::NothingSelected)?,
            _ => return Err(KeyEntryRefusal::SeveralSelected),
        };
        let box_ = oriented_bounds(object);
        let entry = match key {
            EntryKey::Angle => {
                TransformEntry::for_rotate(object, &box_, ResizeDirection::Ne, false)
            }
            // No handle was chosen: the typed size scales about the box centre,
            // as a drag with Shift held would (criterion 57a), and reads no
            // modifier (Ctrl+S is gated, so no Ctrl link either).
            EntryKey::Size => TransformEntry::for_resize(
                object,
                &box_,
                ResizeDirection::Se,
                (true, false),
                self.modes,
            ),
        };
        self.entry = Some(OpenEntry::Transform(entry));
        Ok(())
    }

    /// Acceptance criteria 3, 18, 22, 23, 25-28, 32, 49: the double-click
    /// dispatch. A double-click on a handle (at the *second press's*
    /// position and modifiers, and only if the first press grabbed the same
    /// handle) opens the numeric entry for a rotate or
    /// resize handle and does nothing for a skew handle — neither hands
    /// off to the object's own tool. Anywhere else inside the box of the
    /// sole selected object, the centre handle included, hands off to it;
    /// outside, slice 4's outline hit decides.
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
        // Kept (not consumed): a rapid third press is another double-click on
        // the same handle and keeps its entry.
        let first_press_handle = self.last_press_handle;
        if let Some((object, box_, handle)) =
            Self::handle_at(objects, selection, point, handle_tolerances, shift)
                .filter(|(_, _, handle)| first_press_handle == Some(*handle))
        {
            let entry = match handle {
                EditHandle::Rotate(direction) => Some(OpenEntry::Transform(
                    TransformEntry::for_rotate(object, &box_, direction, shift),
                )),
                EditHandle::Resize(direction) => Some(OpenEntry::Transform(
                    TransformEntry::for_resize(object, &box_, direction, (shift, ctrl), self.modes),
                )),
                EditHandle::Param(param) => {
                    ParamEntry::for_handle(object, &box_, param).map(OpenEntry::Param)
                }
                EditHandle::Skew(_) | EditHandle::Move => None,
            };
            return match entry {
                Some(entry) => {
                    self.entry = Some(entry);
                    SelectDoubleClickOutcome::EntryOpened
                }
                None => SelectDoubleClickOutcome::Ignored,
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
/// hint (criteria 31, 32).
fn hit_outcome(object: &ObjectSnapshot) -> SelectDoubleClickOutcome {
    match object {
        ObjectSnapshot::Path(_) => SelectDoubleClickOutcome::Hit(object.clone()),
        ObjectSnapshot::Primitive(_) => SelectDoubleClickOutcome::EditHint,
    }
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{
        AnchorId, Angle, Document, InnerRatio, Length, NewAnchor, PointCount, RectBounds, Shape,
        StarFrame,
    };

    use super::*;
    use crate::transform_drag::ScaleModes;
    use crate::transform_entry::EntryKind;

    fn rect_at(document: &Document, x: f64, size: f64) -> vecmanf_document_core::NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(x, 0.0),
            width: Length::from_mm(size),
            height: Length::from_mm(size / 2.0),
        })
    }

    fn open(
        document: &Document,
        ids: &[vecmanf_document_core::NodeId],
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

    /// Criterion 59: nothing or several selected refuse and open nothing.
    #[test]
    fn nothing_or_several_selected_refuse_and_open_nothing() {
        let document = Document::new(1);
        let a = rect_at(&document, 0.0, 10.0);
        let b = rect_at(&document, 20.0, 10.0);
        for key in [EntryKey::Angle, EntryKey::Size] {
            let (tool, result) = open(&document, &[], key);
            assert_eq!(result, Err(KeyEntryRefusal::NothingSelected));
            assert!(!tool.has_entry());
            let (tool, result) = open(&document, &[a, b], key);
            assert_eq!(result, Err(KeyEntryRefusal::SeveralSelected));
            assert!(!tool.has_entry());
        }
    }

    fn rect_of(document: &Document, id: vecmanf_document_core::NodeId) -> (Point, Point) {
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
