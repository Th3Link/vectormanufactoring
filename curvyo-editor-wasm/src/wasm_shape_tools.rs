//! The shape tools' `wasm-bindgen` surface (`specs/0003-primitive-shapes`):
//! the polygon/star tool-options bar and "Object to path". A further
//! `impl WasmSession` block (`wasm_api.rs` holds the struct); strings and
//! scalars only (ADR 0001 §5), every method a pass-through to `Session`.

use wasm_bindgen::prelude::*;

use crate::wasm_api::WasmSession;

fn poly_star_mode_from_str(name: &str) -> Result<curvyo_ui_core::PolyStarMode, JsValue> {
    match name {
        "polygon" => Ok(curvyo_ui_core::PolyStarMode::Polygon),
        "star" => Ok(curvyo_ui_core::PolyStarMode::Star),
        other => Err(JsValue::from_str(&format!(
            "unknown polygon/star mode: {other}"
        ))),
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// The polygon/star tool-options bar's current mode: `"polygon"` or
    /// `"star"`.
    #[must_use]
    pub fn poly_star_mode(&self) -> String {
        match self.session.poly_star_mode() {
            curvyo_ui_core::PolyStarMode::Polygon => "polygon".to_string(),
            curvyo_ui_core::PolyStarMode::Star => "star".to_string(),
        }
    }

    /// The mode toggle (acceptance criteria 11 vs. 12).
    ///
    /// # Errors
    /// A `JsValue` if `mode` is neither `"polygon"` nor `"star"`.
    pub fn set_poly_star_mode(&mut self, mode: &str) -> Result<(), JsValue> {
        self.session
            .set_poly_star_mode(poly_star_mode_from_str(mode)?);
        Ok(())
    }

    /// The polygon/star bar's point count for the next shape (acceptance
    /// criterion 10).
    #[must_use]
    pub fn poly_star_point_count(&self) -> u32 {
        self.session.poly_star_point_count().get()
    }

    /// The point-count setting for the next shape (acceptance criterion 10);
    /// it never changes a selected shape (`unified-object-editing` criterion
    /// 29; the Select bar's `set_selected_point_count` does).
    ///
    /// # Errors
    /// A `JsValue` if `count` is outside `3..=1024`.
    pub fn set_poly_star_point_count(&mut self, count: u32) -> Result<(), JsValue> {
        let count = curvyo_document_core::PointCount::new(count)
            .map_err(|err| JsValue::from_str(&format!("{err}")))?;
        self.session.set_poly_star_point_count(count);
        Ok(())
    }

    /// The polygon/star tool-options bar's current ratio (acceptance
    /// criterion 12).
    #[must_use]
    pub fn poly_star_ratio(&self) -> f64 {
        self.session.poly_star_ratio().get()
    }

    /// The ratio setting for the next star (acceptance criteria 12); it never
    /// changes a selected star.
    ///
    /// # Errors
    /// A `JsValue` if `ratio` is outside the open interval `(0, 1)`.
    pub fn set_poly_star_ratio(&mut self, ratio: f64) -> Result<(), JsValue> {
        let ratio = curvyo_document_core::InnerRatio::new(ratio)
            .map_err(|err| JsValue::from_str(&format!("{err}")))?;
        self.session.set_poly_star_ratio(ratio);
        Ok(())
    }

    /// "Object to path" (acceptance criteria 17, 21, 22).
    pub fn convert_selected_to_paths(&mut self) {
        self.session.convert_selected_to_paths();
    }
}
