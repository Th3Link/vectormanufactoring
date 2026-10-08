//! What the Select tool reports about a drag in flight, for drawing it:
//! which handle is dragged, the live pivot, the live move offset, the live
//! resize/rotate/skew result and the skew guide. Split out of `select_tool.rs`
//! (`docs/technical-debt.md`); a child module, so it reads `SelectTool`'s
//! private drag state.

use curvyo_document_core::{Angle, ObjectSnapshot, Point, Vec2};

use super::entry::OpenEntry;
use super::{SelectDrag, SelectTool};
use crate::object_selection::ObjectSelection;
use crate::oriented_box::OrientedBox;
use crate::param_edit::apply_param;
use crate::param_handles::ParamHandle;
use crate::skew_math::skew_frame;
use crate::transform_commit::same_within_tolerance;
use crate::transform_drag::{ParamDragInfo, pivot_for};
use crate::transform_handle_layout::EditHandle;

/// The geometry a release would commit right now, for the blue half of
/// blue-new, black-old (`specs/unified-object-editing/`, criteria 10 to 14):
/// the resolved objects of a move (every selected one), a resize, rotate,
/// skew or parameter drag (the one object). The same resolved snapshots
/// [`SelectTool::pointer_up`] commits, so preview and release cannot
/// disagree.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveEdit {
    /// The resolved objects, uncommitted. In a copy these are the copies,
    /// which carry the ids of their originals: the originals stay where they
    /// are.
    pub objects: Vec<ObjectSnapshot>,
    /// Whether the edit is a copy (a move released with Ctrl down): the
    /// selection boxes and handles stay on the originals while the blue
    /// outline travels alone (criterion 33).
    pub copy: bool,
}

impl SelectTool {
    /// The live edit of the drag in flight with the pointer at `pointer` and
    /// the modifiers as given, re-evaluated from the press every call so a
    /// Shift or Ctrl change with no pointer movement reaches it (criterion
    /// 14). `None` when idle, inside the 3 px dead zone, and when the
    /// resolved objects equal the committed ones within 1e-9 mm and 1e-12 rad
    /// (a drag back to its start shows nothing, criterion 12).
    #[must_use]
    pub fn live_edit(
        &self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        pointer: Point,
        shift: bool,
        ctrl: bool,
    ) -> Option<LiveEdit> {
        let mut copy = false;
        let resolved: Vec<ObjectSnapshot> = if let Some(live) = self.live_move(pointer, shift, ctrl)
        {
            copy = live.copy;
            objects
                .iter()
                .filter(|object| selection.contains(object.id()))
                .map(|object| object.translated(live.offset))
                .collect()
        } else if let Some(live) = self.live_transform(pointer, shift, ctrl) {
            vec![live]
        } else {
            // No drag in flight: a slider edit of the bar, previewed the same
            // way (criterion 10), on the objects it was started against.
            let pending = self.bar_preview.as_ref()?;
            pending
                .ids
                .iter()
                .filter_map(|id| objects.iter().find(|object| object.id() == *id))
                .map(|object| apply_param(object, pending.value))
                .collect()
        };
        let changed = resolved.iter().any(|new| {
            objects
                .iter()
                .find(|old| old.id() == new.id())
                .is_none_or(|old| !same_within_tolerance(old, new))
        });
        changed.then_some(LiveEdit {
            objects: resolved,
            copy,
        })
    }

    /// Whether a move, resize, rotate, skew or parameter drag is in flight. A
    /// drag writes nothing until its release, so the document cannot change
    /// under it.
    #[must_use]
    pub const fn drag_in_flight(&self) -> bool {
        !matches!(self.drag, SelectDrag::None)
    }

    /// Whether the parameter handles are drawn right now: not while the same
    /// object is moved, resized, rotated or skewed by drag (the box can cross
    /// the 72 px threshold mid-drag and they are no part of that gesture), but
    /// yes while idle and during a parameter drag (criterion 7).
    #[must_use]
    pub fn param_handles_visible(&self) -> bool {
        match &self.drag {
            // A marquee or lasso changes nothing under the handles.
            SelectDrag::None | SelectDrag::Marquee(_) | SelectDrag::Lasso(_) => true,
            SelectDrag::Transforming(drag) => matches!(drag.handle, EditHandle::Param(_)),
            SelectDrag::Moving(_) => false,
        }
    }

    /// Which handle is currently being dragged, for the renderer's "solid
    /// fill while dragging" state and the cursor (`docs/design-system.md`).
    /// The centre handle counts while a move that started on it runs.
    #[must_use]
    pub fn dragging_handle(&self) -> Option<EditHandle> {
        match &self.drag {
            SelectDrag::Transforming(drag) => Some(drag.handle),
            SelectDrag::Moving(drag) if drag.from_center => Some(EditHandle::Move),
            SelectDrag::Moving(_)
            | SelectDrag::Marquee(_)
            | SelectDrag::Lasso(_)
            | SelectDrag::None => None,
        }
    }

    /// The active scale/rotate/skew pivot, shown while a drag is in flight
    /// or a numeric entry is open (`docs/design-system.md`'s "Transform
    /// pivot marker") — `shift`'s live state decides which point during a
    /// drag, re-evaluated every frame so the marker jumps the instant the
    /// modifier changes; an open entry's point is fixed when it opened
    /// (criteria 22, 28).
    #[must_use]
    pub fn live_pivot(&self, shift: bool) -> Option<Point> {
        match &self.drag {
            SelectDrag::Transforming(drag) => {
                pivot_for(drag.handle, &drag.start, &drag.start_box, shift)
            }
            SelectDrag::Moving(_) | SelectDrag::Marquee(_) | SelectDrag::Lasso(_) => None,
            SelectDrag::None => match &self.entry {
                Some(OpenEntry::Transform(entry)) => Some(entry.pivot()),
                Some(OpenEntry::Skew(entry)) => Some(entry.pivot()),
                Some(OpenEntry::Param(_) | OpenEntry::Move(_)) | None => None,
            },
        }
    }

    /// The pivot the handle under the pointer *would* use if pressed now
    /// (criterion 55): the marker's preview while Shift is held with no drag
    /// running. `None` for the centre handle.
    #[must_use]
    pub fn hover_pivot(
        object: &ObjectSnapshot,
        box_: &OrientedBox,
        handle: EditHandle,
        shift: bool,
    ) -> Option<Point> {
        pivot_for(handle, object, box_, shift)
    }
    /// The live, uncommitted resize, rotate or skew preview (acceptance
    /// criteria 14, 22 of slice 5; 40 here): the object as it would commit
    /// right now, re-evaluated from the drag-start snapshot every call so
    /// `shift`/`ctrl`'s live state is always reflected. `None` unless such a
    /// drag is in flight and past the dead zone.
    #[must_use]
    pub fn live_transform(
        &self,
        current: Point,
        shift: bool,
        ctrl: bool,
    ) -> Option<ObjectSnapshot> {
        match &self.drag {
            SelectDrag::Transforming(drag) if drag.origin.is_active_at(current) => {
                Some(drag.resolve(current, shift, ctrl))
            }
            SelectDrag::Transforming(_)
            | SelectDrag::Moving(_)
            | SelectDrag::Marquee(_)
            | SelectDrag::Lasso(_)
            | SelectDrag::None => None,
        }
    }

    /// The facts a corner radius drag's readout and followers need with the
    /// pointer at `current` (`specs/rectangle-corner-radii/` criteria 4, 7, 23):
    /// `None` unless such a drag is in flight and past the dead zone.
    #[must_use]
    pub fn live_param_drag(&self, current: Point) -> Option<ParamDragInfo> {
        match &self.drag {
            SelectDrag::Transforming(drag) if drag.origin.is_active_at(current) => {
                drag.param_info(current)
            }
            SelectDrag::Transforming(_)
            | SelectDrag::Moving(_)
            | SelectDrag::Marquee(_)
            | SelectDrag::Lasso(_)
            | SelectDrag::None => None,
        }
    }

    /// Whether the corner radius drag in flight changes all four corners
    /// (decided at the press): the other three knobs then take the hover
    /// ground. `None` when no corner radius drag runs.
    #[must_use]
    pub fn corner_drag_changes_all(&self) -> Option<bool> {
        match &self.drag {
            SelectDrag::Transforming(drag)
                if matches!(drag.handle, EditHandle::Param(ParamHandle::CornerRadius(_))) =>
            {
                Some(!drag.unlinked)
            }
            _ => None,
        }
    }

    /// The live skew angle for the readout (criterion 40): `None` unless a
    /// skew drag is in flight and past the dead zone.
    #[must_use]
    pub fn live_skew_angle(&self, current: Point, shift: bool, ctrl: bool) -> Option<Angle> {
        match &self.drag {
            SelectDrag::Transforming(drag) if drag.origin.is_active_at(current) => {
                drag.skew_angle_at(current, shift, ctrl)
            }
            SelectDrag::Transforming(_)
            | SelectDrag::Moving(_)
            | SelectDrag::Marquee(_)
            | SelectDrag::Lasso(_)
            | SelectDrag::None => None,
        }
    }

    /// For a skew drag in flight: the two end points of the dashed guide
    /// along the line that stays fixed (criterion 56) — the fixed edge, or
    /// the line through the box center under `shift` — extended
    /// `extend_mm` past each end of the box.
    #[must_use]
    pub fn skew_guide(&self, shift: bool, extend_mm: f64) -> Option<(Point, Point)> {
        let SelectDrag::Transforming(drag) = &self.drag else {
            return None;
        };
        let EditHandle::Skew(side) = drag.handle else {
            return None;
        };
        let frame = skew_frame(&drag.start_box, side, shift);
        let (sin, cos) = drag.start_box.angle.as_radians().sin_cos();
        // The fixed line runs along `u` for an x skew, along `v` otherwise.
        let (direction, length) = if frame.along_u {
            (Vec2::new(cos, sin), drag.start_box.width())
        } else {
            (Vec2::new(-sin, cos), drag.start_box.height())
        };
        let half = direction.scaled(length / 2.0 + extend_mm);
        Some((
            frame.fixed_point.translated(half.negated()),
            frame.fixed_point.translated(half),
        ))
    }
}
