//! The rulers' and the status bar's `wasm-bindgen` surface
//! (`specs/0015-document-size-and-rulers/`): the tick layout of one ruler strip
//! for the live view, the numbers that tell the host the view changed, and the
//! unit-aware status texts. A further `impl WasmSession` block; scalars,
//! number arrays and strings only (ADR 0001 §5), every method a pass-through
//! to `Session`.

use curvyo_ui_core::{RulerAxis, RulerLayout};
use wasm_bindgen::prelude::*;

use crate::wasm_api::WasmSession;

/// One ruler strip's ticks and labels, in strip pixels from its start. The
/// host draws exactly this and computes nothing.
#[wasm_bindgen]
pub struct RulerView {
    layout: RulerLayout,
}

#[wasm_bindgen]
impl RulerView {
    /// The major tick positions, from just before the strip's start to just
    /// after its end.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn majors(&self) -> Vec<f64> {
        self.layout.majors.iter().map(|major| major.px).collect()
    }

    /// The minor tick positions (four between two majors).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn minors(&self) -> Vec<f64> {
        self.layout.minors.clone()
    }

    /// The position of the tick at value 0, if it lies on the strip. It is
    /// also in `majors`; the host draws it 2 px wide.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn origin(&self) -> Option<f64> {
        self.layout.origin_px
    }

    /// The position of the major tick each label belongs to; a label starts
    /// four pixels after its tick.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn label_ticks(&self) -> Vec<f64> {
        self.layout
            .labels
            .iter()
            .map(|label| label.tick_px)
            .collect()
    }

    /// The label texts, in the order of `label_ticks`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn label_texts(&self) -> Vec<String> {
        self.layout
            .labels
            .iter()
            .map(|label| label.text.clone())
            .collect()
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// The layout of the ruler along the top (`horizontal`) or left edge,
    /// `length_px` CSS pixels long, with labels in the display unit.
    /// `digit_px` is the widest digit's advance and `minus_px` the minus sign's,
    /// both in the label font.
    #[must_use]
    pub fn ruler_view(
        &self,
        horizontal: bool,
        length_px: f64,
        digit_px: f64,
        minus_px: f64,
    ) -> RulerView {
        let axis = if horizontal {
            RulerAxis::Horizontal
        } else {
            RulerAxis::Vertical
        };
        RulerView {
            layout: self
                .session
                .ruler_layout(axis, length_px, digit_px, minus_px),
        }
    }

    /// CSS pixels per document millimetre, with `view_origin_x` and
    /// `view_origin_y`: the host redraws the rulers only when one of the
    /// three, the unit or a strip's size changed.
    #[must_use]
    pub fn view_scale(&self) -> f64 {
        self.session.view().scale()
    }

    /// The document x (mm) at the canvas's left edge.
    #[must_use]
    pub fn view_origin_x(&self) -> f64 {
        self.session.screen_to_document(0.0, 0.0).x
    }

    /// The document y (mm) at the canvas's top edge.
    #[must_use]
    pub fn view_origin_y(&self) -> f64 {
        self.session.screen_to_document(0.0, 0.0).y
    }

    /// The display unit's symbol: `"mm"`, `"cm"` or `"in"`.
    #[must_use]
    pub fn display_unit(&self) -> String {
        self.session.display_unit().symbol().to_string()
    }

    /// The status bar's cursor readout for a document point in millimetres,
    /// for example `"x: 12.3  y: 45.6 mm"`.
    #[must_use]
    pub fn cursor_text(&self, x_mm: f64, y_mm: f64) -> String {
        self.session.cursor_text(x_mm, y_mm)
    }

    /// The status bar's size readout, for example `"210.0 × 297.0 mm"`.
    #[must_use]
    pub fn size_text(&self) -> String {
        self.session.size_text()
    }
}
