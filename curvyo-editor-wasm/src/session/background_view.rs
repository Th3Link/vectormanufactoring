//! The Background block's state as the flat record the host reads (ADR 0001 §5):
//! one read after every change, no editing logic
//! (`specs/0040-document-background` criteria 15 to 21).

use curvyo_document_core::{BackgroundPaint, DocumentBackground};
use curvyo_ui_core::{ValueScale, hex_text};

/// What the Background block shows. Colours are `0xRRGGBB`, the opacity a percent
/// (the host rounds for display). The paint decides which rows are shown; the
/// colour and opacity are those of a drag in flight when there is one.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundView {
    /// `"none"` or `"solid"`, the stored paint.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub paint: String,
    /// The colour, `0xRRGGBB`.
    pub color: u32,
    /// The colour and alpha as `#RRGGBBAA`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub hex: String,
    /// The opacity, percent.
    pub opacity: f64,
    /// The opacity as the field shows it.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub opacity_text: String,
    /// The bar position of the opacity.
    pub opacity_bar: f64,
    /// The reset icon of the opacity shows.
    pub opacity_resettable: bool,
    /// The largest opacity that may be typed, percent.
    pub opacity_typed_max: f64,
    /// The opacity reset target as the tooltip words it.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub opacity_default_text: String,
    /// The eyedropper is picking for the background.
    pub picking: bool,
}

impl BackgroundView {
    /// The record for `shown`, with the rows of the stored `paint`.
    #[must_use]
    pub fn new(paint: BackgroundPaint, shown: DocumentBackground, picking: bool) -> Self {
        let scale = ValueScale::Opacity;
        let percent = shown.opacity.get() * 100.0;
        let field = scale.shown(Some(percent));
        Self {
            paint: paint.name().to_string(),
            color: u32::from(shown.color.r) << 16
                | u32::from(shown.color.g) << 8
                | u32::from(shown.color.b),
            hex: hex_text(shown.color, shown.opacity),
            opacity: percent,
            opacity_text: field.text,
            opacity_bar: field.bar,
            opacity_resettable: field.resettable,
            opacity_typed_max: scale.typed_max(),
            opacity_default_text: scale.default_text(),
            picking,
        }
    }
}
