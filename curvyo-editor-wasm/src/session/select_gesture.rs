//! What `Session` shows for the Select tool's marquee and lasso
//! (`specs/0014-advanced-selection/`): the box or line overlay, the legend text
//! near the pointer and the cursor. A pure read of the tool's live gesture
//! and the cached modifiers, so a key change reaches all three in the same
//! frame with the pointer at rest (criterion 14).

use curvyo_render_core::MarqueeOverlay;
use curvyo_ui_core::{
    GestureKind, GestureShape, LiveGesture, MarqueeMode, PressTarget, SelectionCombine,
};

use super::shapes::LiveReadout;
use super::{Session, Tool};

impl Session {
    /// The gesture in flight with the live pointer and the cached modifiers.
    fn live_gesture(&self) -> Option<LiveGesture> {
        if self.tool != Tool::Select {
            return None;
        }
        self.select.live_gesture(self.pointer_position?, self.held)
    }

    /// The marquee box or lasso line to draw this frame, once the drag has
    /// left its 3 px dead zone.
    pub(super) fn marquee_overlay(&self) -> Option<MarqueeOverlay> {
        Some(match self.live_gesture()?.shape {
            GestureShape::Box { from, to, mode } => MarqueeOverlay::Box {
                from,
                to,
                contain: mode == MarqueeMode::Contain,
            },
            GestureShape::Line(points) => MarqueeOverlay::Lasso(points),
        })
    }

    /// The modifier-state legend of a marquee or lasso past its dead zone:
    /// mode and combine, "Touch · Replace", "Contain · −Remove",
    /// "Touch (line) · +Add", at the pointer.
    pub(super) fn gesture_readout(&self) -> Option<LiveReadout> {
        let live = self.live_gesture()?;
        let mode = match live.shape {
            GestureShape::Box {
                mode: MarqueeMode::Touch,
                ..
            } => "Touch",
            GestureShape::Box {
                mode: MarqueeMode::Contain,
                ..
            } => "Contain",
            GestureShape::Line(_) => "Touch (line)",
        };
        let combine = match live.combine {
            SelectionCombine::Replace => "Replace",
            SelectionCombine::Add => "+Add",
            SelectionCombine::Remove => "\u{2212}Remove",
        };
        Some(LiveReadout {
            text: format!("{mode} \u{b7} {combine}"),
            anchor: self.pointer_position?,
        })
    }

    /// The cursor of a selection gesture, or `None` when the ordinary cursor
    /// rules apply: the crosshair from the press of a marquee, the lasso
    /// glyph from the press of a lasso and, with no button down, while Alt is
    /// held (the next press arms one). The kind is locked at the press, so
    /// pressing or releasing Alt mid-drag changes nothing here.
    pub(super) fn gesture_cursor(&self) -> Option<&'static str> {
        match self.select.gesture_kind() {
            Some(GestureKind::Marquee) => Some("crosshair"),
            Some(GestureKind::Lasso) => Some("lasso"),
            None if !self.select.drag_in_flight() && PressTarget::arms_lasso(self.held) => {
                Some("lasso")
            }
            None => None,
        }
    }
}
