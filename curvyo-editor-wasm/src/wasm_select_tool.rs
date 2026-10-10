//! The Select tool's `wasm-bindgen` surface (`specs/0005-object-transform`,
//! `specs/0008-object-transform-refinements`, `specs/0009-unified-object-editing`):
//! the handle hint chip, the typed numeric entry and the Select tool's
//! switches. A further `impl WasmSession` block (`wasm_api.rs` holds the
//! struct); strings and scalars only (ADR 0001 §5), every method a
//! pass-through to `Session`.

use curvyo_document_core::ViewTransform;
use wasm_bindgen::prelude::*;

use crate::wasm_api::WasmSession;

/// `wasm-bindgen`'s JS-facing mirror of [`crate::session::EntryView`]
/// (`specs/0008-object-transform-refinements/adrs.md`, "wasm surface": strings
/// and scalars only): the typed numeric entry chip's content and where it
/// belongs. `handle_*`/`center_*` are canvas-relative CSS pixels, already
/// converted; the host puts the chip outward of the handle, along the line
/// from the center through it.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct TransformEntryView {
    kind: String,
    fields: Vec<crate::session::EntryFieldView>,
    pub linked: bool,
    scope: String,
    pub handle_x: f64,
    pub handle_y: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub glyph_reach: f64,
    /// The chip goes by the box centre instead of outward from the handle.
    pub at_centre: bool,
    /// The chip edits a multi-selection.
    pub selection: bool,
}

#[wasm_bindgen]
impl TransformEntryView {
    /// `"angle"`, `"size"`, `"radius"` (a polygon or star's outer radius),
    /// `"corner-radius"`, `"inner-ratio"` or `"skew"`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    /// The muted second row of a corner radius entry ("All four corners" or
    /// "This corner only"); empty for every other entry.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn scope(&self) -> String {
        self.scope.clone()
    }

    /// How many fields the chip has (one or two).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn field_count(&self) -> u32 {
        u32::try_from(self.fields.len()).unwrap_or(0)
    }

    /// Field `index`'s visible label ("W", "H", "r"; empty for the angle).
    #[must_use]
    pub fn field_label(&self, index: u32) -> String {
        self.field(index)
            .map(|f| f.label.to_string())
            .unwrap_or_default()
    }

    /// Field `index`'s accessible name ("Width", "Height", "Outer radius", "Angle").
    #[must_use]
    pub fn field_name(&self, index: u32) -> String {
        self.field(index)
            .map(|f| f.accessible_name.to_string())
            .unwrap_or_default()
    }

    /// The text field `index` opens with.
    #[must_use]
    pub fn field_prefill(&self, index: u32) -> String {
        self.field(index)
            .map(|f| f.prefill.clone())
            .unwrap_or_default()
    }

    /// Whether field `index` can be edited.
    #[must_use]
    pub fn field_editable(&self, index: u32) -> bool {
        self.field(index).is_some_and(|f| f.editable)
    }
}

impl TransformEntryView {
    fn field(&self, index: u32) -> Option<&crate::session::EntryFieldView> {
        self.fields.get(usize::try_from(index).ok()?)
    }

    /// Converts a document-space [`crate::session::EntryView`] to this
    /// screen-pixel mirror — [`WasmSession::transform_entry`] is the one
    /// caller.
    fn from_document_space(entry: crate::session::EntryView, view: ViewTransform) -> Self {
        let (handle_x, handle_y) = view.document_to_screen(entry.handle);
        let (center_x, center_y) = view.document_to_screen(entry.center);
        Self {
            kind: entry.kind.to_string(),
            fields: entry.fields,
            linked: entry.linked,
            scope: entry.scope.unwrap_or_default().to_string(),
            handle_x,
            handle_y,
            center_x,
            center_y,
            glyph_reach: entry.glyph_reach_px,
            at_centre: entry.at_centre,
            selection: entry.selection,
        }
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// Which hint the handle under the pointer earns (`""`, `"resize-edge"`,
    /// `"resize-corner"`, `"resize-corner-uniform"`, `"rotate-corner"`,
    /// `"rotate-side"`, `"skew"`, `"move"`, `"param-radius"` or
    /// `"param-inner"`) — for the host's 600 ms hover
    /// chip. Call after every [`WasmSession::pointer_hover`].
    #[must_use]
    pub fn handle_hint(&self) -> String {
        self.session.handle_hint()
    }

    /// The lines of the hint chip of a corner radius knob under the pointer
    /// (state-dependent, see [`Session::corner_hint_lines`]); empty on any
    /// other handle.
    #[must_use]
    pub fn corner_hint_lines(&self) -> Vec<String> {
        self.session.corner_hint_lines()
    }

    /// What a screen reader says about a multi-selection ("4 objects selected,
    /// 46.2 by 18.7 mm"); empty for fewer than two selected objects.
    #[must_use]
    pub fn selection_announcement(&self) -> String {
        self.session.selection_announcement()
    }

    /// Whether releasing the drag in flight may take long (a move or group
    /// transform of many objects): the host shows the `wait` cursor and lets it
    /// paint before it calls `pointer_up`.
    #[must_use]
    pub fn release_is_slow(&self) -> bool {
        self.session.release_is_slow()
    }

    /// The typed numeric entry to show, or `undefined` (criteria 18, 25, 26
    /// of `object-transform-refinements`). Call after every pointer release
    /// and tool or selection change.
    #[must_use]
    pub fn transform_entry(&self) -> Option<TransformEntryView> {
        let view = self.session.view();
        self.session
            .transform_entry()
            .map(|entry| TransformEntryView::from_document_space(entry, view))
    }

    /// For linked fields (criterion 29): the text the other field takes after
    /// field `field` became `text`, or `undefined`.
    #[must_use]
    pub fn transform_entry_linked(&self, field: u32, text: &str) -> Option<String> {
        self.session
            .transform_entry_linked(usize::try_from(field).ok()?, text)
    }

    /// Enter in the entry chip: `"committed"`, `"unchanged"` (both close the
    /// chip), or `"invalid:<field>:<reason>"` with the reason `number`,
    /// `positive`, `negative`, `ratio-range`, `skew-range` or `too-large` (the
    /// chip stays open; criteria 19, 21, 27, 30, 31; 18, 19 of
    /// `unified-object-editing`; 11 of `edit-interaction-polish`).
    /// `last_edited` is the index of the field edited last.
    pub fn commit_transform_entry(
        &mut self,
        first: &str,
        second: &str,
        last_edited: u32,
    ) -> String {
        let last = usize::try_from(last_edited).unwrap_or(0);
        crate::wasm_move_entry::outcome_code(
            self.session.commit_transform_entry(first, second, last),
        )
    }

    /// Closes the numeric entry without writing (criterion 20): Escape, a
    /// blur, a window blur. Idempotent.
    pub fn cancel_transform_entry(&mut self) {
        self.session.cancel_transform_entry();
    }

    /// Acceptance criterion 6's "remove rounding" action, from the Select
    /// bar (`unified-object-editing` criterion 21) or the Rectangle tool's:
    /// zeroes the radius of every selected rectangle. A no-op in every other
    /// tool.
    pub fn remove_corner_rounding(&mut self) {
        self.session.remove_corner_rounding();
    }

    /// The Select tool's "Scale stroke width" switch (`object-transform`
    /// AC 26-31). Off in every new session; not persisted.
    #[must_use]
    pub fn scale_stroke_width(&self) -> bool {
        self.session.scale_stroke_width()
    }

    /// Sets the "Scale stroke width" switch for the next resize drag.
    pub fn set_scale_stroke_width(&mut self, on: bool) {
        self.session.set_scale_stroke_width(on);
    }
}
