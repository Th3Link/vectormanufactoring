//! The Properties panel's Document section and the status bar texts, as
//! `wasm-bindgen` calls (`specs/0015-document-size-and-rulers/`): what the
//! panel shows, the size fields, the display unit, Fit to content, and the
//! unit-aware status texts. A further `impl WasmSession` block; strings,
//! booleans and numbers only (ADR 0001 §5), every method a pass-through to
//! `Session`.

use curvyo_document_core::{DisplayUnit, Orientation};
use wasm_bindgen::prelude::*;

use crate::session::{
    DocumentPresetsRecord, DocumentSide, FitOutcome, PanelTabsRecord, SizeOutcome,
};
use crate::wasm_api::WasmSession;

fn side_from_str(name: &str) -> Option<DocumentSide> {
    match name {
        "width" => Some(DocumentSide::Width),
        "height" => Some(DocumentSide::Height),
        _ => None,
    }
}

fn outcome_word(outcome: SizeOutcome) -> String {
    match outcome {
        SizeOutcome::Committed => "committed",
        SizeOutcome::Unchanged => "unchanged",
        SizeOutcome::Invalid => "invalid:number",
    }
    .to_string()
}

#[wasm_bindgen]
impl WasmSession {
    /// What the Properties panel shows: the body (`"document"`, `"style"` or
    /// `"empty"`), the active tab and the strip. Reading it lets the tab rule
    /// see a change of selection, so the tab changes in the same frame.
    pub fn panel_view(&self) -> PanelTabsRecord {
        PanelTabsRecord::new(&self.session.panel_view())
    }

    /// A press on the tab `name` (`"document"` or `"style"`): `false` when
    /// nothing changed (the dimmed Style tab, an unknown name).
    pub fn press_panel_tab(&mut self, name: &str) -> bool {
        self.session.press_panel_tab(name)
    }

    /// Shift+Ctrl+F: Style with a selection, Document without.
    pub fn panel_shortcut_style(&mut self) {
        self.session.panel_shortcut_style();
    }

    /// Shift+Ctrl+D: the Document tab.
    pub fn panel_shortcut_document(&mut self) {
        self.session.panel_shortcut_document();
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
        outcome_word(self.session.set_document_side(side, text))
    }

    /// The presets part of the Document section: the subject line, the pressed
    /// orientation and every button in file order. Read in the same sync as the
    /// size fields.
    #[must_use]
    pub fn document_presets_view(&self) -> DocumentPresetsRecord {
        DocumentPresetsRecord::new(&self.session.document_presets_view())
    }

    /// A press on the preset button `id`: `"committed"`, `"unchanged"` or
    /// `"invalid:number"` (an unknown id).
    pub fn apply_document_preset(&mut self, id: &str) -> String {
        outcome_word(self.session.apply_document_preset(id))
    }

    /// A press on an orientation item, `"portrait"` or `"landscape"`:
    /// `"committed"` or `"unchanged"`.
    pub fn set_document_orientation(&mut self, orientation: &str) -> String {
        let wanted = match orientation {
            "portrait" => Orientation::Portrait,
            "landscape" => Orientation::Landscape,
            _ => return "unchanged".to_string(),
        };
        outcome_word(self.session.set_document_orientation(wanted))
    }

    /// The Fit to content button: `"fitted"`, `"already-fits"`, `"empty"`,
    /// `"blocked"` (the Pen has an unfinished path) or `"too-large:<message>"`.
    pub fn fit_document(&mut self) -> String {
        match self.session.fit_document() {
            FitOutcome::Fitted => "fitted".to_string(),
            FitOutcome::AlreadyFits => "already-fits".to_string(),
            FitOutcome::Empty => "empty".to_string(),
            FitOutcome::Blocked => "blocked".to_string(),
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
