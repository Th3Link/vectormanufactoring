//! The move drag's modifier badges over `wasm-bindgen`
//! (`specs/edit-interaction-polish/`, criteria 26, 27, 33): a second
//! `impl WasmSession` block, so `wasm_api.rs` does not grow. Scalars and
//! strings only (ADR 0001 §5).

use vecmanf_ui_core::Axis;
use wasm_bindgen::prelude::*;

use crate::wasm_api::WasmSession;

/// Which badges the DOM shows by the pointer: `copy_badge` is the plus badge,
/// `lock` is `""` (no lock badge), `"x"` or `"y"` (the axis the move is locked
/// to).
#[wasm_bindgen(getter_with_clone)]
#[derive(Debug, Clone)]
pub struct MoveIndicatorsView {
    pub copy_badge: bool,
    pub lock: String,
}

#[wasm_bindgen]
impl WasmSession {
    /// The modifier badges right now: a pure read of the cached modifiers, the
    /// pointer, the press classification and the drag. Call after every
    /// pointer event and every modifier change, so a badge appears and
    /// vanishes in the frame of its key.
    #[must_use]
    pub fn move_indicators(&self) -> MoveIndicatorsView {
        let indicators = self.session.move_indicators();
        MoveIndicatorsView {
            copy_badge: indicators.copy_badge,
            lock: match indicators.lock {
                Some(Axis::X) => "x",
                Some(Axis::Y) => "y",
                None => "",
            }
            .to_string(),
        }
    }
}
