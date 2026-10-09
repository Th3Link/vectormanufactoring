//! The Select bar's `wasm-bindgen` surface (`specs/0009-unified-object-editing`,
//! criteria 21 to 23): a second `impl WasmSession` block, so `wasm_api.rs`
//! does not grow. Strings and scalars only (ADR 0001 §5); every method is a
//! direct pass-through to `Session`, which holds the orchestration, over
//! `curvyo-ui-core`'s rules.

use curvyo_ui_core::{BarValue, EntryOutcome, InvalidReason, SelectBarState, clamped_ratio};
use wasm_bindgen::prelude::*;

use crate::wasm_api::WasmSession;

/// `wasm-bindgen`'s JS-facing mirror of [`curvyo_ui_core::SelectBarState`]:
/// which kind controls the Select bar shows and their values. A `*_mixed`
/// flag means the selected objects differ (the field is empty with the
/// placeholder "Mixed"); the value beside it is then meaningless.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct SelectBarView {
    /// "Radius" is shown (the selection holds a rectangle).
    pub radius_shown: bool,
    /// The rectangles hold different effective radii.
    pub radius_mixed: bool,
    /// The effective radius, millimetres.
    pub radius: f64,
    /// The stored radius exceeds what the rectangle allows ("limited").
    pub radius_limited: bool,
    /// The stored radius, millimetres, for the tooltip of "limited".
    pub radius_stored: f64,
    /// One rectangle with unequal corners is selected: `radius_tl` to
    /// `radius_bl` are its effective radii for the "Mixed" tooltip.
    pub radius_corners_shown: bool,
    /// Top-left effective radius, millimetres.
    pub radius_tl: f64,
    /// Top-right effective radius, millimetres.
    pub radius_tr: f64,
    /// Bottom-right effective radius, millimetres.
    pub radius_br: f64,
    /// Bottom-left effective radius, millimetres.
    pub radius_bl: f64,
    /// "Remove rounding" is shown.
    pub remove_rounding_shown: bool,
    /// "Remove rounding" would change something.
    pub remove_rounding_enabled: bool,
    /// "Points" is shown (the selection holds a polygon or star).
    pub points_shown: bool,
    /// The polygons and stars hold different point counts.
    pub points_mixed: bool,
    /// The point count.
    pub points: u32,
    /// "Ratio" is shown (the selection holds a star).
    pub ratio_shown: bool,
    /// The stars hold different ratios.
    pub ratio_mixed: bool,
    /// The inner ratio.
    pub ratio: f64,
    /// "Object to path" is shown.
    pub object_to_path_shown: bool,
}

impl From<SelectBarState> for SelectBarView {
    fn from(state: SelectBarState) -> Self {
        let (radius_shown, radius_mixed, radius) = match state.radius {
            None => (false, false, 0.0),
            Some(BarValue::Mixed) => (true, true, 0.0),
            Some(BarValue::Uniform(length)) => (true, false, length.as_mm()),
        };
        let (points_shown, points_mixed, points) = match state.points {
            None => (false, false, 0),
            Some(BarValue::Mixed) => (true, true, 0),
            Some(BarValue::Uniform(count)) => (true, false, count),
        };
        let (ratio_shown, ratio_mixed, ratio) = match state.ratio {
            None => (false, false, 0.0),
            Some(BarValue::Mixed) => (true, true, 0.0),
            Some(BarValue::Uniform(value)) => (true, false, value),
        };
        Self {
            radius_shown,
            radius_mixed,
            radius,
            radius_limited: state.radius_limited.is_some(),
            radius_stored: state
                .radius_limited
                .map_or(0.0, curvyo_document_core::Length::as_mm),
            radius_corners_shown: state.radius_corners.is_some(),
            radius_tl: state.radius_corners.map_or(0.0, |c| c.tl.as_mm()),
            radius_tr: state.radius_corners.map_or(0.0, |c| c.tr.as_mm()),
            radius_br: state.radius_corners.map_or(0.0, |c| c.br.as_mm()),
            radius_bl: state.radius_corners.map_or(0.0, |c| c.bl.as_mm()),
            remove_rounding_shown: state.remove_rounding_shown,
            remove_rounding_enabled: state.remove_rounding_enabled,
            points_shown,
            points_mixed,
            points,
            ratio_shown,
            ratio_mixed,
            ratio,
            object_to_path_shown: state.object_to_path,
        }
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// What the Select bar shows for the current selection. Call after every
    /// pointer release and tool or selection change, and after every bar edit.
    #[must_use]
    pub fn select_bar_state(&self) -> SelectBarView {
        self.session.select_bar_state().into()
    }

    /// The Select tool's "Scale corner radius" switch (criterion 23). Off in
    /// every new session; not persisted.
    #[must_use]
    pub fn scale_corner_radius(&self) -> bool {
        self.session.scale_corner_radius()
    }

    /// Sets the "Scale corner radius" switch for the next resize. Clicking it
    /// closes an open numeric entry without writing.
    pub fn set_scale_corner_radius(&mut self, on: bool) {
        self.session.set_scale_corner_radius(on);
    }

    /// The Select tool's "Link corners" switch (`specs/0013-rectangle-corner-radii/`
    /// criterion 2). On in every new session; not persisted.
    #[must_use]
    pub fn link_corners(&self) -> bool {
        self.session.link_corners()
    }

    /// Sets the "Link corners" switch for the next corner radius drag or
    /// entry. Writes nothing; an open numeric entry closes without writing.
    pub fn set_link_corners(&mut self, on: bool) {
        self.session.set_link_corners(on);
    }

    /// Removes the "max" notice of a limited typed radius, 1.5 s after it
    /// appeared.
    pub fn clear_limit_notice(&mut self) {
        self.session.clear_limit_notice();
    }

    /// Enter in the bar's "Radius" field (criterion 21a): `"committed"`,
    /// `"unchanged"`, or `"invalid:number"` / `"invalid:negative"` (the field
    /// stays open, marked invalid, and nothing is written).
    pub fn set_selected_radius(&mut self, text: &str) -> String {
        match self.session.set_selected_radius_text(text) {
            EntryOutcome::Committed => "committed".to_string(),
            EntryOutcome::Unchanged => "unchanged".to_string(),
            EntryOutcome::Invalid { reason, .. } => format!(
                "invalid:{}",
                match reason {
                    InvalidReason::Negative => "negative",
                    _ => "number",
                }
            ),
        }
    }

    /// The "Points" field and stepper: one commit for every selected polygon
    /// and star.
    ///
    /// # Errors
    /// A `JsValue` if `count` is outside `3..=1024`.
    pub fn set_selected_point_count(&mut self, count: u32) -> Result<(), JsValue> {
        let count = curvyo_document_core::PointCount::new(count)
            .map_err(|err| JsValue::from_str(&format!("{err}")))?;
        self.session.set_selected_point_count(count);
        Ok(())
    }

    /// The "Ratio" field's discrete commit: one commit for every selected
    /// star. A value outside 0.01 to 0.99 is limited to it.
    pub fn set_selected_ratio(&mut self, ratio: f64) {
        self.session.set_selected_ratio(clamped_ratio(ratio));
    }

    /// The "Ratio" slider's live preview, on every tick: writes nothing, the
    /// stars show in blue over their unchanged old shape.
    pub fn preview_selected_ratio(&mut self, ratio: f64) {
        self.session.preview_selected_ratio(clamped_ratio(ratio));
    }

    /// Commits the slider edit pending, as one commit, on the slider's
    /// release, key-up or blur.
    pub fn commit_selected_ratio(&mut self) {
        self.session.commit_selected_ratio();
    }
}
