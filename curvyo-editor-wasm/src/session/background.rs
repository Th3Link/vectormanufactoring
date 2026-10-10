//! `Session`'s document background (`specs/0040-document-background`
//! criteria 14 to 25, `adrs.md` decision 9): the commands the Background block
//! issues and the ephemeral preview a drag or a held key draws in place of the
//! stored value. The rules live in `curvyo_ui_core` (`parse_hex`, `hsv_to_rgb`,
//! `ValueScale::Opacity`); every edit ends in `Document::set_background`, one
//! commit that writes only the register that changed.

use curvyo_document_core::{BackgroundPaint, DocumentBackground};
use curvyo_ui_core::{
    Grid, StyleEntryError, ValueScale, hsv_to_rgb, opacity_from_percent, parse_hex,
    parse_opacity_percent,
};

use super::Session;
use super::background_view::BackgroundView;

impl Session {
    /// The background the canvas and the Background block show now: the preview
    /// of a drag or a held key when there is one, else the stored value.
    pub(super) fn shown_background(&self) -> DocumentBackground {
        self.background_preview
            .unwrap_or_else(|| self.document.background())
    }

    /// What the Background block shows (criteria 15 to 21). Which rows are shown
    /// follows the stored paint; the colour and opacity follow the preview.
    #[must_use]
    pub fn background_view(&self) -> BackgroundView {
        BackgroundView::new(
            self.document.background().paint,
            self.shown_background(),
            self.colour_pick_target() == Some(curvyo_ui_core::PaintTarget::Background),
        )
    }

    /// The Paint group (criterion 17): one commit. `false` when the paint already is `paint`.
    pub fn set_background_paint(&mut self, paint: BackgroundPaint) -> bool {
        self.edit_background(|stored| DocumentBackground { paint, ..stored })
    }

    /// Enter or Tab in the hex field (criterion 18): `Ok(true)` after one commit,
    /// `Ok(false)` when the colour is the stored one. A 3 or 6 digit form keeps the alpha.
    ///
    /// # Errors
    /// [`StyleEntryError::Hex`] for text that is no colour; nothing is written.
    pub fn set_background_hex(&mut self, text: &str) -> Result<bool, StyleEntryError> {
        let typed = parse_hex(text)?;
        Ok(self.edit_background(|stored| DocumentBackground {
            color: typed.color,
            opacity: typed.opacity.unwrap_or(stored.opacity),
            ..stored
        }))
    }

    /// Enter or Tab in the Opacity field (criterion 19): a whole percent stored as
    /// `N / 100`, like an object's.
    ///
    /// # Errors
    /// [`StyleEntryError::Percent`] for text that is no number from 0 to 100.
    pub fn set_background_opacity_text(&mut self, text: &str) -> Result<bool, StyleEntryError> {
        let opacity = parse_opacity_percent(text)?;
        Ok(self.edit_background(|stored| DocumentBackground { opacity, ..stored }))
    }

    /// The reset slot of the Opacity field (`0017` criterion 61): one commit to 100 %.
    pub fn reset_background_opacity(&mut self) -> bool {
        let opacity = opacity_from_percent(ValueScale::Opacity.default_value());
        self.edit_background(|stored| DocumentBackground { opacity, ..stored })
    }

    /// A tick of a drag in the saturation/value area or the hue slider (criterion
    /// 20): shows the colour without writing. The alpha stays.
    pub fn preview_background_hsv(&mut self, hue: f64, saturation: f64, value: f64) {
        self.background_preview = Some(DocumentBackground {
            color: hsv_to_rgb(hue, saturation, value),
            ..self.shown_background()
        });
    }

    /// A tick of a drag on the Opacity field (`0017` criteria 36 to 47): position
    /// `p` (`0` to `1`) on the scale, rounded to `grid`, shown without writing.
    pub fn preview_background_opacity(&mut self, p: f64, grid: Grid) {
        // A tick that is not a number shows and writes nothing.
        if !p.is_finite() {
            return;
        }
        let scale = ValueScale::Opacity;
        let percent = scale.round(scale.value_at(p), grid);
        self.show_background_opacity(percent);
    }

    /// An arrow key on the Opacity field (`0017` criterion 43): `steps` steps from
    /// the shown value, previewed; the key-up commits.
    pub fn step_background_opacity(&mut self, steps: i32, grid: Grid) {
        let shown = self.shown_background().opacity.get() * 100.0;
        self.show_background_opacity(ValueScale::Opacity.step(shown, steps, grid));
    }

    fn show_background_opacity(&mut self, percent: f64) {
        self.background_preview = Some(DocumentBackground {
            opacity: opacity_from_percent(percent),
            ..self.shown_background()
        });
    }

    /// The release or key-up (criterion 22): writes the previewed value as one
    /// commit. Writes nothing after Escape, or when it equals the stored value.
    pub fn commit_background_preview(&mut self) {
        if let Some(preview) = self.background_preview.take() {
            // Only the colour: the paint captured when the drag started is not written back.
            let paint = self.document.background().paint;
            let _ = self
                .document
                .set_background(DocumentBackground { paint, ..preview });
        }
    }

    /// Escape or a system cancel during a drag: the stored value shows again and the
    /// release then writes nothing.
    pub fn cancel_background_preview(&mut self) {
        self.background_preview = None;
    }

    /// Writes `edit` applied to the stored background as one commit, after a
    /// pending preview was committed so that an edit made while a drag was still
    /// open builds on what the drag showed. `false` when nothing changed.
    fn edit_background(
        &mut self,
        edit: impl FnOnce(DocumentBackground) -> DocumentBackground,
    ) -> bool {
        self.commit_background_preview();
        self.document
            .set_background(edit(self.document.background()))
    }
}
