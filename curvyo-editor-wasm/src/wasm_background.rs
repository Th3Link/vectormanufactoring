//! The Document section's Background block as `wasm-bindgen` calls
//! (`specs/0040-document-background`): one read and the commands of its controls.
//! A further `impl WasmSession` block; strings and scalars only (ADR 0001 §5),
//! every method a pass-through to `Session`. The eyedropper button is the Style
//! panel's `begin_colour_pick("background")`.

use curvyo_document_core::BackgroundPaint;
use curvyo_ui_core::{Grid, StyleEntryError};
use wasm_bindgen::prelude::*;

use crate::session::BackgroundView;
use crate::wasm_api::WasmSession;

fn outcome(result: Result<bool, StyleEntryError>) -> String {
    match result {
        Ok(true) => "committed".to_string(),
        Ok(false) => "unchanged".to_string(),
        Err(error) => format!("invalid:{}", error.code()),
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// What the Background block shows. Call after every change of the document and
    /// every edit of the block.
    #[must_use]
    pub fn background_view(&self) -> BackgroundView {
        self.session.background_view()
    }

    /// A press on the Paint group, `"none"` or `"solid"`: one commit. `false` when
    /// the paint already is the pressed one or the word is unknown.
    pub fn set_background_paint(&mut self, name: &str) -> bool {
        BackgroundPaint::from_name(name)
            .is_some_and(|paint| self.session.set_background_paint(paint))
    }

    /// Enter or Tab in the hex field: `"committed"`, `"unchanged"` or
    /// `"invalid:hex"`.
    pub fn set_background_hex(&mut self, text: &str) -> String {
        outcome(self.session.set_background_hex(text))
    }

    /// Enter or Tab in the Opacity field: `"committed"`, `"unchanged"` or
    /// `"invalid:percent"`.
    pub fn set_background_opacity_text(&mut self, text: &str) -> String {
        outcome(self.session.set_background_opacity_text(text))
    }

    /// The reset slot of the Opacity field: one commit to 100 %. `false` when it
    /// already is.
    pub fn reset_background_opacity(&mut self) -> bool {
        self.session.reset_background_opacity()
    }

    /// A tick of a drag in the saturation/value area or the hue slider: shows the
    /// colour of hue `hue` (degrees), saturation and value (`0` to `1`) without
    /// writing.
    pub fn preview_background_hsv(&mut self, hue: f64, saturation: f64, value: f64) {
        self.session.preview_background_hsv(hue, saturation, value);
    }

    /// A tick of a drag on the Opacity field: position `p` (`0` to `1`), rounded to
    /// `grid` (`"normal"`, `"coarse"` or `"fine"`), shown without writing.
    pub fn preview_background_opacity(&mut self, p: f64, grid_name: &str) {
        if let Some(grid) = Grid::from_name(grid_name) {
            self.session.preview_background_opacity(p, grid);
        }
    }

    /// An arrow key on the Opacity field: `steps` steps from the shown value,
    /// previewed; the key-up commits.
    pub fn step_background_opacity(&mut self, steps: i32, grid_name: &str) {
        if let Some(grid) = Grid::from_name(grid_name) {
            self.session.step_background_opacity(steps, grid);
        }
    }

    /// The release, key-up or blur of a drag: one commit, nothing after Escape.
    pub fn commit_background_preview(&mut self) {
        self.session.commit_background_preview();
    }

    /// Escape during a drag: the stored background shows again and the release then
    /// writes nothing.
    pub fn cancel_background_preview(&mut self) {
        self.session.cancel_background_preview();
    }
}
