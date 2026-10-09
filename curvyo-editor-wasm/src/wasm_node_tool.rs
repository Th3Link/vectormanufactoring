//! The Node tool's `wasm-bindgen` surface (`specs/0002-path-node-editing`,
//! `specs/0006-path-merge-split-and-node-types`): node-kind conversion, line
//! and curve, join, split, insert and the contextual toolbar's enabled state.
//! A further `impl WasmSession` block (`wasm_api.rs` holds the struct);
//! strings and scalars only (ADR 0001 §5), every method a pass-through to
//! `Session`.

use curvyo_document_core::AnchorKind;
use curvyo_ui_core::NodeToolbarState as SessionNodeToolbarState;
use wasm_bindgen::prelude::*;

use crate::wasm_api::WasmSession;

/// `"corner" | "symmetric" | "asymmetric"`
/// (`specs/0006-path-merge-split-and-node-types/adrs.md`: "the wasm
/// binding string (`"smooth"` in `wasm_api.rs`...) is not persisted and
/// is renamed outright" — the old `"smooth"` spelling is gone here too,
/// not kept as an accepted alias the way the on-disk tag is).
fn kind_from_str(name: &str) -> Result<AnchorKind, JsValue> {
    match name {
        "corner" => Ok(AnchorKind::Corner),
        "symmetric" => Ok(AnchorKind::Symmetric),
        "asymmetric" => Ok(AnchorKind::Asymmetric),
        other => Err(JsValue::from_str(&format!("unknown anchor kind: {other}"))),
    }
}

/// `wasm-bindgen`'s JS-facing mirror of [`curvyo_ui_core::NodeToolbarState`]
/// — a plain `bool`-fields struct needs no getter methods, unlike a type
/// `wasm-bindgen` can't expose by value. See that type's own doc comment
/// for why independent `bool`s, not an enum.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct NodeToolbarState {
    pub can_insert: bool,
    pub can_delete: bool,
    pub can_convert_to_corner: bool,
    pub can_convert_to_symmetric: bool,
    pub can_convert_to_asymmetric: bool,
    pub can_make_line: bool,
    pub can_make_curve: bool,
    pub can_join: bool,
    pub can_split: bool,
    pub compound_only: bool,
}

impl From<SessionNodeToolbarState> for NodeToolbarState {
    fn from(state: SessionNodeToolbarState) -> Self {
        Self {
            can_insert: state.can_insert,
            can_delete: state.can_delete,
            can_convert_to_corner: state.can_convert_to_corner,
            can_convert_to_symmetric: state.can_convert_to_symmetric,
            can_convert_to_asymmetric: state.can_convert_to_asymmetric,
            can_make_line: state.can_make_line,
            can_make_curve: state.can_make_curve,
            can_join: state.can_join,
            can_split: state.can_split,
            compound_only: state.compound_only,
        }
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// The node-kind conversion actions: `"corner"`, `"symmetric"` or
    /// `"asymmetric"` (`specs/0006-path-merge-split-and-node-types/
    /// specification.md` acceptance criteria 1, 2, 4, 5, 16).
    ///
    /// # Errors
    /// A `JsValue` if `kind` is none of those.
    pub fn convert_selected(&mut self, kind: &str) -> Result<(), JsValue> {
        self.session.convert_selected(kind_from_str(kind)?);
        Ok(())
    }

    /// Acceptance criterion 14's "make line".
    pub fn make_line(&mut self) {
        self.session.make_line();
    }

    /// Acceptance criterion 14's "make curve".
    pub fn make_curve(&mut self) {
        self.session.make_curve();
    }

    /// Acceptance criteria 8-11: the "Join" toolbar/context-menu button.
    pub fn join_selected(&mut self) {
        self.session.join_selected();
    }

    /// Acceptance criteria 12-15: the "Split" toolbar/context-menu
    /// button.
    pub fn split_selected(&mut self) {
        self.session.split_selected();
    }

    /// The contextual toolbar's "Insert node" button: splits the
    /// currently selected segment at its midpoint.
    pub fn insert_selected(&mut self) {
        self.session.insert_selected();
    }

    /// Which contextual-toolbar buttons are currently enabled
    /// (`specification.md`'s UX notes: "Buttons disable (not hide) when
    /// nothing selected/applicable").
    #[must_use]
    pub fn node_toolbar_state(&self) -> NodeToolbarState {
        self.session.node_toolbar_state().into()
    }
}
