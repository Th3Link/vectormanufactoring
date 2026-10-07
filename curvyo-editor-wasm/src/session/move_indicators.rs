//! What `Session` reports about a move drag in flight, for the draw list, the
//! readout and the DOM badges (`specs/edit-interaction-polish/adrs.md`,
//! decision 5; criteria 26, 27, 33, 37).

use vecmanf_document_core::{ObjectSnapshot, Point, Vec2};
use vecmanf_render_core::{LockedAxis, MoveAxes};
use vecmanf_ui_core::{Axis, classify_press, oriented_bounds};

use super::shapes::LiveReadout;
use super::{Session, Tool};

/// The canvas size assumed before the host's first size report, CSS pixels.
const FALLBACK_CANVAS_PX: f64 = 4096.0;

/// The two modifier badges of a move, a pure read of the cached modifiers, the
/// pointer, the press classification and the drag: the DOM places them and
/// carries no rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveIndicators {
    /// The plus badge: a copy drag runs, or a press with Ctrl held here would
    /// start a move.
    pub copy_badge: bool,
    /// The lock badge: the axis a move past the dead zone is locked to.
    pub lock: Option<Axis>,
}

impl Session {
    /// The badges to show right now. The plus badge follows Ctrl and the
    /// press's own classification (never on empty canvas, where Ctrl belongs
    /// to the marquee); it stays while Ctrl is held after Escape cancelled a
    /// drag, because it is recomputed from the held key. The lock badge shows
    /// only while a move drag past the dead zone is locked, never before the
    /// press (before it Shift means "add to selection").
    #[must_use]
    pub fn move_indicators(&self) -> MoveIndicators {
        let none = MoveIndicators {
            copy_badge: false,
            lock: None,
        };
        if self.tool != Tool::Select {
            return none;
        }
        let Some(pointer) = self.pointer_position else {
            return none;
        };
        let lock = self
            .select
            .live_move(pointer, self.select_shift_held, self.select_ctrl_held)
            .and_then(|live| live.axis);
        let copy_badge = self.select_ctrl_held
            && if self.select.drag_in_flight() {
                self.select.move_in_flight()
            } else {
                classify_press(
                    &self.objects(),
                    &self.selection,
                    pointer,
                    self.segment_tolerance(),
                    self.transform_handle_tolerances(),
                    self.select_shift_held,
                )
                .begins_move()
            };
        MoveIndicators { copy_badge, lock }
    }

    /// The origin axes of an axis-locked move: through the selection's start
    /// centre (where the centre handle was at the press), spanning the whole
    /// viewport. `None` unless a move past the dead zone is locked.
    pub(super) fn move_axes_in(&self, objects: &[ObjectSnapshot]) -> Option<MoveAxes> {
        if self.tool != Tool::Select {
            return None;
        }
        let pointer = self.pointer_position?;
        let live = self
            .select
            .live_move(pointer, self.select_shift_held, self.select_ctrl_held)?;
        let locked = match live.axis? {
            Axis::X => LockedAxis::Horizontal,
            Axis::Y => LockedAxis::Vertical,
        };
        let centre = self.selection_centre(objects)?;
        let (width, height) = self.viewport.canvas_size();
        let (width, height) = if width > 0.0 && height > 0.0 {
            (width, height)
        } else {
            (FALLBACK_CANVAS_PX, FALLBACK_CANVAS_PX)
        };
        let view = self.view();
        let (top_left, bottom_right) = (
            view.screen_to_document(0.0, 0.0),
            view.screen_to_document(width, height),
        );
        Some(MoveAxes {
            horizontal: (
                Point::new(top_left.x, centre.y),
                Point::new(bottom_right.x, centre.y),
            ),
            vertical: (
                Point::new(centre.x, top_left.y),
                Point::new(centre.x, bottom_right.y),
            ),
            locked,
            device_pixel_ratio: self.device_pixel_ratio,
        })
    }

    /// The centre of the box around the selected objects' oriented boxes as
    /// they are committed: the centre handle's position for one object.
    fn selection_centre(&self, objects: &[ObjectSnapshot]) -> Option<Point> {
        let corners: Vec<Point> = objects
            .iter()
            .filter(|object| self.selection.contains(object.id()))
            .flat_map(|object| oriented_bounds(object).document_corners())
            .collect();
        let (first, rest) = corners.split_first()?;
        let (low, high) = rest.iter().fold((*first, *first), |(low, high), p| {
            (
                Point::new(low.x.min(p.x), low.y.min(p.y)),
                Point::new(high.x.max(p.x), high.y.max(p.y)),
            )
        });
        Some(Point::new(
            f64::midpoint(low.x, high.x),
            f64::midpoint(low.y, high.y),
        ))
    }

    /// The readout of a move drag past the dead zone: the offset a release
    /// would commit, after any axis lock, with " Copy" in copy mode.
    pub(super) fn move_readout(&self) -> Option<LiveReadout> {
        let anchor = self.pointer_position?;
        let live = self
            .select
            .live_move(anchor, self.select_shift_held, self.select_ctrl_held)?;
        Some(LiveReadout {
            text: move_readout_text(live.offset, live.copy),
            anchor,
        })
    }
}

/// `Δ 12.5, −3.0 mm`, with ` Copy` for a copy: X right, Y down, one decimal,
/// a real minus sign (U+2212) in the readout only.
fn move_readout_text(offset: Vec2, copy: bool) -> String {
    let suffix = if copy { " Copy" } else { "" };
    format!(
        "Δ {}, {} mm{suffix}",
        signed_mm(offset.x),
        signed_mm(offset.y)
    )
}

/// One decimal; a value that rounds to zero shows no sign.
fn signed_mm(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded == 0.0 {
        "0.0".to_string()
    } else if rounded < 0.0 {
        format!("\u{2212}{:.1}", -rounded)
    } else {
        format!("{rounded:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_readout_uses_a_real_minus_and_one_decimal() {
        assert_eq!(
            move_readout_text(Vec2::new(12.5, -3.0), false),
            "Δ 12.5, \u{2212}3.0 mm"
        );
        assert_eq!(
            move_readout_text(Vec2::new(30.0, 0.0), true),
            "Δ 30.0, 0.0 mm Copy"
        );
        assert_eq!(
            move_readout_text(Vec2::new(-0.04, 0.04), false),
            "Δ 0.0, 0.0 mm",
            "no negative zero"
        );
    }
}
