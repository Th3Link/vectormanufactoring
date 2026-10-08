//! The Style panel's `wasm-bindgen` surface (`specs/0007-stroke-and-fill-
//! styling`, criteria 5, 6, 13, 14, 24, 36, 37): a `impl WasmSession` block of
//! its own, so `wasm_api.rs` does not grow. Strings and scalars only (ADR 0001
//! §5); every method is a direct pass-through to `Session`, which holds the
//! orchestration, over `curvyo-ui-core`'s rules.

use curvyo_document_core::Color;
use curvyo_ui_core::{DashChoice, StyleField, cap_from_name, fill_mode_from_name, join_from_name};
use wasm_bindgen::prelude::*;

use crate::session::StylePanelView;
use crate::wasm_api::WasmSession;

fn field(name: &str) -> Option<StyleField> {
    StyleField::from_name(name)
}

#[wasm_bindgen]
impl WasmSession {
    /// What the Style panel shows for the active tool and selection. Call
    /// after every pointer release, tool or selection change, and panel edit.
    #[must_use]
    pub fn style_panel_view(&self) -> StylePanelView {
        self.session.style_panel_view()
    }

    /// Enter or Tab in a typed field. `field_name` is `"stroke-width"`,
    /// `"stroke-color"`, `"stroke-opacity"`, `"fill-color"` or
    /// `"fill-opacity"`. Returns `"committed"` (one commit for every edited
    /// object), `"unchanged"` (nothing to edit), or `"invalid:<code>"` with
    /// the code `hex`, `hex8`, `percent` or `width`: the field stays open and
    /// nothing is written.
    pub fn set_style_text(&mut self, field_name: &str, text: &str) -> String {
        let Some(field) = field(field_name) else {
            return "unchanged".to_string();
        };
        match self.session.set_style_text(field, text) {
            Ok(true) => "committed".to_string(),
            Ok(false) => "unchanged".to_string(),
            Err(error) => format!("invalid:{}", error.code()),
        }
    }

    /// A colour area or hue slider tick: shows `rgb` (`0xRRGGBB`) on the edited
    /// objects without writing. `field_name` is `"stroke-color"` or
    /// `"fill-color"`.
    pub fn preview_style_color(&mut self, field_name: &str, rgb: u32) {
        if let Some(field) = field(field_name) {
            let color = Color {
                r: ((rgb >> 16) & 0xFF) as u8,
                g: ((rgb >> 8) & 0xFF) as u8,
                b: (rgb & 0xFF) as u8,
            };
            self.session.preview_style_color(field, color);
        }
    }

    /// An opacity slider tick (percent): shows it without writing.
    /// `field_name` is `"stroke-opacity"` or `"fill-opacity"`.
    pub fn preview_style_opacity(&mut self, field_name: &str, percent: f64) {
        if let Some(field) = field(field_name) {
            self.session.preview_style_opacity(field, percent);
        }
    }

    /// The release, key-up or blur of a panel drag: one commit to the objects
    /// the drag started on, nothing after Escape.
    pub fn commit_style_preview(&mut self) {
        self.session.commit_style_preview();
    }

    /// Escape during a panel drag: the objects return to their committed
    /// style and the release then writes nothing.
    pub fn cancel_style_preview(&mut self) {
        self.session.cancel_style_preview();
    }

    /// The stroke Paint switch: one commit.
    pub fn set_stroke_paint(&mut self, on: bool) {
        self.session.set_stroke_paint(on);
    }

    /// The Dash select: `"solid"`, `"dash"`, `"dot"` or `"dash-dot"`; any
    /// other word writes nothing.
    pub fn set_stroke_dash(&mut self, name: &str) {
        if let Some(choice) = DashChoice::from_name(name) {
            self.session.set_stroke_dash(choice);
        }
    }

    /// The Join group: `"miter"`, `"round"` or `"bevel"`.
    pub fn set_stroke_join(&mut self, name: &str) {
        if let Some(join) = join_from_name(name) {
            self.session.set_stroke_join(join);
        }
    }

    /// The Cap group: `"butt"`, `"round"` or `"square"`.
    pub fn set_stroke_cap(&mut self, name: &str) {
        if let Some(cap) = cap_from_name(name) {
            self.session.set_stroke_cap(cap);
        }
    }

    /// The Fill type row: `"none"`, `"solid"`, `"linear"` or `"radial"`.
    pub fn set_fill_mode(&mut self, name: &str) {
        if let Some(mode) = fill_mode_from_name(name) {
            self.session.set_fill_mode(mode);
        }
    }

    /// The properties panel is about to open (`-280`) or close (`+280`): the
    /// canvas resize that follows keeps the view's top-left origin, so the
    /// document does not move on screen. Call just before the layout changes.
    pub fn keep_view_origin_for_panel_toggle(&mut self, width_delta_css_px: f64) {
        self.session
            .keep_view_origin_for_width_change(width_delta_css_px);
    }

    /// Whether a canvas pointer press is in flight.
    #[must_use]
    pub fn pointer_is_down(&self) -> bool {
        self.session.is_pointer_down()
    }
}
