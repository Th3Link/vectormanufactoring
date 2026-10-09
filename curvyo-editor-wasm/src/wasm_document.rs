//! The Properties panel's Document section and the status bar texts, as
//! `wasm-bindgen` calls (`specs/0015-document-size-and-rulers/`): what the
//! panel shows, the size fields, the display unit, Fit to content, and the
//! unit-aware status texts. A further `impl WasmSession` block; strings,
//! booleans and numbers only (ADR 0001 §5), every method a pass-through to
//! `Session`.

use curvyo_document_core::DisplayUnit;
use curvyo_ui_core::PanelContent;
use wasm_bindgen::prelude::*;

use crate::session::{DocumentSide, FitOutcome, SizeOutcome};
use crate::wasm_api::WasmSession;

fn side_from_str(name: &str) -> Option<DocumentSide> {
    match name {
        "width" => Some(DocumentSide::Width),
        "height" => Some(DocumentSide::Height),
        _ => None,
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// What the Properties panel shows: `"document"` (nothing selected),
    /// `"style"` (something selected) or `"empty"` (the Pen has an unfinished
    /// path).
    #[must_use]
    pub fn panel_content(&self) -> String {
        match self.session.panel_content() {
            PanelContent::Document => "document",
            PanelContent::Style => "style",
            PanelContent::Empty => "empty",
        }
        .to_string()
    }

    /// The display unit's symbol: `"mm"`, `"cm"` or `"in"`.
    #[must_use]
    pub fn display_unit(&self) -> String {
        self.session.display_unit().symbol().to_string()
    }

    /// Sets the display unit (`"mm"`, `"cm"` or `"in"`): one commit that moves
    /// nothing. `false` for an unknown symbol or the unit already shown.
    pub fn set_display_unit(&mut self, symbol: &str) -> bool {
        DisplayUnit::from_symbol(symbol).is_some_and(|unit| self.session.set_display_unit(unit))
    }

    /// Whether the document has objects, so Fit to content is offered.
    #[must_use]
    pub fn document_has_objects(&self) -> bool {
        self.session.has_objects()
    }

    /// The text of the `"width"` or `"height"` field in the display unit.
    #[must_use]
    pub fn document_side_text(&self, side: &str) -> String {
        side_from_str(side).map_or_else(String::new, |side| self.session.side_text(side))
    }

    /// The message of a refused size, the limits in the display unit.
    #[must_use]
    pub fn document_side_message(&self) -> String {
        self.session.side_message()
    }

    /// Enter or Tab in the `"width"` or `"height"` field: `"committed"`,
    /// `"unchanged"` or `"invalid:number"`.
    pub fn set_document_side(&mut self, side: &str, text: &str) -> String {
        let Some(side) = side_from_str(side) else {
            return "unchanged".to_string();
        };
        match self.session.set_document_side(side, text) {
            SizeOutcome::Committed => "committed",
            SizeOutcome::Unchanged => "unchanged",
            SizeOutcome::Invalid => "invalid:number",
        }
        .to_string()
    }

    /// The Fit to content button: `"fitted"`, `"already-fits"`, `"empty"` or
    /// `"too-large:<message>"`.
    pub fn fit_document(&mut self) -> String {
        match self.session.fit_document() {
            FitOutcome::Fitted => "fitted".to_string(),
            FitOutcome::AlreadyFits => "already-fits".to_string(),
            FitOutcome::Empty => "empty".to_string(),
            FitOutcome::TooLarge(message) => format!("too-large:{message}"),
        }
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
