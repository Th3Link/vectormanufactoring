//! What the Select tool reports about a drag in flight, for drawing it:
//! which handle is dragged, the live pivot, the live move offset, the live
//! resize/rotate/skew result and the skew guide. Split out of `select_tool.rs`
//! (`docs/technical-debt.md`); a child module, so it reads `SelectTool`'s
//! private drag state.

use vecmanf_document_core::{Angle, ObjectSnapshot, Point, Vec2};

use super::{SelectDrag, SelectTool};
use crate::oriented_box::OrientedBox;
use crate::skew_math::skew_frame;
use crate::transform_drag::pivot_for;
use crate::transform_entry::TransformEntry;
use crate::transform_handle_layout::TransformHandle;

impl SelectTool {
    /// Which handle is currently being dragged, for the renderer's "solid
    /// fill while dragging" state and the cursor (`docs/design-system.md`).
    /// The centre handle counts while a move that started on it runs.
    #[must_use]
    pub fn dragging_handle(&self) -> Option<TransformHandle> {
        match &self.drag {
            SelectDrag::Transforming(drag) => Some(drag.handle),
            SelectDrag::Moving {
                from_center: true, ..
            } => Some(TransformHandle::Move),
            SelectDrag::Moving { .. } | SelectDrag::None => None,
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
            SelectDrag::Moving { .. } => None,
            SelectDrag::None => self.entry.as_ref().map(TransformEntry::pivot),
        }
    }

    /// The pivot the handle under the pointer *would* use if pressed now
    /// (criterion 55): the marker's preview while Shift is held with no drag
    /// running. `None` for the centre handle.
    #[must_use]
    pub fn hover_pivot(
        object: &ObjectSnapshot,
        box_: &OrientedBox,
        handle: TransformHandle,
        shift: bool,
    ) -> Option<Point> {
        pivot_for(handle, object, box_, shift)
    }
    /// The live, uncommitted move offset while a drag is in flight — the
    /// Select tool's own counterpart to the shape tools' `live_shape`
    /// (acceptance criterion 20's "live"). `None` when idle, or while the
    /// pointer is still inside the dead zone — a press-and-release there
    /// must write nothing (`specs/0002-path-node-editing/adrs.md`'s rule).
    #[must_use]
    pub fn live_offset(&self, current_point: Point) -> Option<Vec2> {
        match &self.drag {
            SelectDrag::Moving { origin, .. } if origin.is_active_at(current_point) => {
                Some(origin.down_at.vector_to(current_point))
            }
            SelectDrag::Moving { .. } | SelectDrag::Transforming(_) | SelectDrag::None => None,
        }
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
            SelectDrag::Transforming(_) | SelectDrag::Moving { .. } | SelectDrag::None => None,
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
            SelectDrag::Transforming(_) | SelectDrag::Moving { .. } | SelectDrag::None => None,
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
        let TransformHandle::Skew(side) = drag.handle else {
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
