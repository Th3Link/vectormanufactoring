//! `Session`'s eyedropper (`specs/0017-style-panel-rework` criteria 22 to 27,
//! `adrs.md` decision 3): while picking is on, a primary press on the canvas
//! takes the colour under it (`curvyo_ui_core::pick_colour`) and writes it as
//! the stroke or fill colour of the style scope in one commit; no tool sees
//! the press. Pan and zoom are navigation and keep working. The state is
//! ephemeral: a tool change, Escape, a right press or the host ends it.

use curvyo_document_core::{Point, StyleEdit};
use curvyo_ui_core::{PaintTarget, PickedColour, hex_text, pick_colour};

use super::Session;

/// What the eyedropper holds.
#[derive(Debug, Default)]
pub(super) struct ColourPick {
    /// The paint being picked for; `None` when picking is off.
    target: Option<PaintTarget>,
    /// The colour a click at the pointer would take.
    hover: Option<PickedColour>,
    /// The release that follows a pick belongs to no gesture.
    swallow_release: bool,
}

impl Session {
    /// Starts picking a colour from the drawing for `target` (the Pick
    /// button). Pressing it again ends picking.
    pub fn begin_colour_pick(&mut self, target: PaintTarget) {
        self.flush_style_preview();
        if self.colour_pick.target == Some(target) {
            self.end_colour_pick();
            return;
        }
        self.colour_pick.target = Some(target);
        self.colour_pick.hover = None;
    }

    /// Ends picking and writes nothing.
    pub fn end_colour_pick(&mut self) {
        self.colour_pick = ColourPick {
            swallow_release: self.colour_pick.swallow_release,
            ..ColourPick::default()
        };
    }

    /// The paint being picked for, if picking is on.
    #[must_use]
    pub const fn colour_pick_target(&self) -> Option<PaintTarget> {
        self.colour_pick.target
    }

    /// The colour a click at the pointer would take, as `#RRGGBBAA`, and the
    /// paint it comes from; `None` while picking is off or where nothing is
    /// painted.
    #[must_use]
    pub fn colour_pick_hover(&self) -> Option<(String, PaintTarget)> {
        self.colour_pick
            .hover
            .map(|picked| (hex_text(picked.color, picked.opacity), picked.paint))
    }

    /// A primary press while picking: takes the colour and ends picking, or,
    /// where nothing is painted, writes nothing and keeps picking. Returns
    /// whether picking consumed the press.
    pub(super) fn colour_pick_press(&mut self, point: Point) -> bool {
        let Some(target) = self.colour_pick.target else {
            self.colour_pick.swallow_release = false;
            return false;
        };
        self.colour_pick.swallow_release = true;
        if let Some(picked) = pick_colour(&self.objects(), point, self.object_tolerance()) {
            let edit = match target {
                PaintTarget::Stroke => StyleEdit::StrokeRgba(picked.color, picked.opacity),
                PaintTarget::Fill => StyleEdit::FillRgba(picked.color, picked.opacity),
            };
            self.apply_style_edit(&edit);
            self.end_colour_pick();
        }
        true
    }

    /// The pointer moved while picking: remembers what a click would take.
    /// Returns whether picking consumed the move (no tool hover then).
    pub(super) fn colour_pick_move(&mut self, point: Point) -> bool {
        if self.colour_pick.target.is_none() {
            return false;
        }
        self.colour_pick.hover = pick_colour(&self.objects(), point, self.object_tolerance());
        true
    }

    /// A pan or zoom moved the document under the pointer at canvas pixel
    /// `(screen_x, screen_y)`: the colour a click would take is read again.
    pub(super) fn refresh_colour_pick_hover(&mut self, screen_x: f64, screen_y: f64) {
        if self.colour_pick.target.is_some() {
            let point = self.screen_to_document(screen_x, screen_y);
            self.colour_pick_move(point);
        }
    }

    /// The release of a press picking consumed: nothing for a tool to finish.
    pub(super) fn colour_pick_release(&mut self) -> bool {
        std::mem::take(&mut self.colour_pick.swallow_release)
    }
}
