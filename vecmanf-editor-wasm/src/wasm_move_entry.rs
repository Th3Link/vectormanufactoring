//! The typed move's `wasm-bindgen` surface
//! (`specs/edit-interaction-polish/`, criteria 15 to 25): a second
//! `impl WasmSession` block, so `wasm_api.rs` does not grow. Strings and
//! scalars only (ADR 0001 §5); every method passes straight through to
//! `Session`, which holds the orchestration, over `vecmanf-ui-core`'s rules.

use vecmanf_ui_core::{EntryOutcome, InvalidReason};
use wasm_bindgen::prelude::*;

use crate::session::MoveEntryView;
use crate::wasm_api::WasmSession;

/// The string an entry commit reports: `"committed"`, `"unchanged"` or
/// `"invalid:<field>:<reason>"` (the entry stays open). Shared by the size,
/// angle, skew and move chips.
pub(crate) fn outcome_code(outcome: EntryOutcome) -> String {
    match outcome {
        EntryOutcome::Committed => "committed".to_string(),
        EntryOutcome::Unchanged => "unchanged".to_string(),
        EntryOutcome::Invalid { field, reason } => format!(
            "invalid:{field}:{}",
            match reason {
                InvalidReason::NotANumber => "number",
                InvalidReason::NotPositive => "positive",
                InvalidReason::Negative => "negative",
                InvalidReason::RatioRange => "ratio-range",
                InvalidReason::SkewRange => "skew-range",
                InvalidReason::TooLarge => "too-large",
            }
        ),
    }
}

/// `wasm-bindgen`'s JS-facing mirror of [`MoveEntryView`]: the typed move
/// chip's anchor and prefill. `center_*` are canvas-relative CSS pixels,
/// already converted; the chip opens 16 px right of and below it.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct MoveEntryChip {
    pub center_x: f64,
    pub center_y: f64,
    relative: [String; 2],
    absolute: [String; 2],
}

#[wasm_bindgen]
impl MoveEntryChip {
    /// The text X (`0`) or Y (`1`) opens with in Relative mode.
    #[must_use]
    pub fn relative_prefill(&self, axis: u32) -> String {
        self.relative
            .get(axis as usize)
            .cloned()
            .unwrap_or_default()
    }

    /// The text X (`0`) or Y (`1`) shows in Absolute mode while untouched: the
    /// object's current top-left, one decimal.
    #[must_use]
    pub fn absolute_prefill(&self, axis: u32) -> String {
        self.absolute
            .get(axis as usize)
            .cloned()
            .unwrap_or_default()
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// The typed move chip to show, or `undefined` (criteria 15, 18, 56).
    /// Call after every pointer release, key press and tool or selection
    /// change.
    #[must_use]
    pub fn move_entry(&self) -> Option<MoveEntryChip> {
        let MoveEntryView {
            center,
            relative_prefill,
            absolute_prefill,
        } = self.session.move_entry()?;
        let (center_x, center_y) = self.session.view().document_to_screen(center);
        Some(MoveEntryChip {
            center_x,
            center_y,
            relative: relative_prefill,
            absolute: absolute_prefill,
        })
    }

    /// Enter in the move chip: `first` and `second` are the X and Y texts and
    /// `absolute` the chip's mode (criteria 19 to 22, 25). Returns
    /// `"committed"`, `"unchanged"` (both close the chip) or
    /// `"invalid:<field>:number"` (the chip stays open).
    pub fn commit_move_entry(&mut self, first: &str, second: &str, absolute: bool) -> String {
        outcome_code(self.session.commit_move_entry(first, second, absolute))
    }
}
