//! The move drag's modifier badges over `wasm-bindgen`
//! (`specs/0010-edit-interaction-polish/`, criteria 26, 27, 33): a second
//! `impl WasmSession` block, so `wasm_api.rs` does not grow. Scalars and
//! strings only (ADR 0001 §5).

use curvyo_ui_core::Axis;
use wasm_bindgen::prelude::*;

use crate::wasm_api::WasmSession;

/// Which badges the DOM shows by the pointer: `copy_badge` is the plus badge,
/// `remove_badge` the minus badge of a Ctrl marquee or lasso, `lock` is `""` (no lock badge), `"x"` or `"y"` (the axis the move is locked
/// to).
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct MoveIndicatorsView {
    copy_badge: bool,
    remove_badge: bool,
    lock: &'static str,
}

#[wasm_bindgen]
impl MoveIndicatorsView {
    /// Whether the plus badge shows.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn copy_badge(&self) -> bool {
        self.copy_badge
    }

    /// Whether the minus badge shows (`advanced-selection`).
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn remove_badge(&self) -> bool {
        self.remove_badge
    }

    /// The lock badge's axis: `""`, `"x"` or `"y"`.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn lock(&self) -> String {
        self.lock.to_string()
    }
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
            remove_badge: indicators.remove_badge,
            lock: match indicators.lock {
                Some(Axis::X) => "x",
                Some(Axis::Y) => "y",
                None => "",
            },
        }
    }
}
