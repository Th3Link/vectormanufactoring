//! The keyboard's `wasm-bindgen` surface (`specs/0010-edit-interaction-polish/`,
//! Parts D and F): a second `impl WasmSession` block, so `wasm_api.rs` does
//! not grow. Strings and scalars only (ADR 0001 §5); every method passes
//! straight through to `Session`, which holds the key table, the gate and the
//! Escape cascade.

use wasm_bindgen::prelude::*;

use crate::session::KeyInput;
use crate::wasm_api::{LiveReadout, WasmSession};

#[wasm_bindgen]
impl WasmSession {
    /// One key press: `key` is `KeyboardEvent.key`, `ctrl` already folds in
    /// Cmd, `repeat` is the event's auto-repeat flag and `dom_blocked` is
    /// true when focus is in a text field, select, button, switch or
    /// contenteditable element, an IME composition is running, or Space is
    /// held. Returns what happened: `"ignored"`, `"tool"`, `"entry"`,
    /// `"deleted"`, `"pen"`, `"escape-entry"`, `"escape-drag"`,
    /// `"escape-state"`, `"escape-tool"`, `"escape-none"`,
    /// `"hint-select-one"`, `"hint-select-first"`, `"hint-path-only"`,
    /// `"hint-too-far"`, `"select-all"`, `"nudge"` (an arrow key moved the
    /// selection) or `"nudge-new"` (and opened a new run of held-key events).
    /// `time_ms` is `KeyboardEvent.timeStamp`. The host prevents the page's
    /// default for every result except `"ignored"` and re-reads the session
    /// state for them.
    #[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)] // one scalar per DOM fact
    pub fn key_down(
        &mut self,
        key: &str,
        shift: bool,
        ctrl: bool,
        alt: bool,
        repeat: bool,
        dom_blocked: bool,
        time_ms: f64,
    ) -> String {
        self.session
            .key_down_at(
                KeyInput {
                    key,
                    shift,
                    ctrl,
                    alt,
                    repeat,
                    dom_blocked,
                },
                time_ms,
            )
            .code()
            .to_string()
    }

    /// The move readout of the nudge step that is running ("Δ 3.0, 0.0 mm"),
    /// anchored at the selection's centre in canvas pixels, or `undefined`
    /// before any nudge.
    #[must_use]
    pub fn nudge_readout(&self) -> Option<LiveReadout> {
        let view = self.session.view();
        self.session
            .nudge_readout()
            .map(|readout| LiveReadout::from_document_space(readout, view))
    }

    /// What a screen reader hears when the running nudge step ends ("Moved 11
    /// mm right."), or an empty string before any nudge.
    #[must_use]
    pub fn nudge_announcement(&self) -> String {
        self.session.nudge_announcement()
    }

    /// The browser took the pointer away (`pointercancel`, a window blur):
    /// cancels any drag in flight and forgets the pressed button.
    pub fn pointer_cancelled(&mut self) {
        self.session.pointer_cancelled();
    }

    /// How many objects are selected: the rail's tooltips say "(Esc, R)"
    /// while the Select tool has one (criterion 62).
    #[must_use]
    pub fn selection_count(&self) -> u32 {
        u32::try_from(self.session.selected_object_count()).unwrap_or(u32::MAX)
    }
}
