//! The properties panel's `wasm-bindgen` surface (`specs/0007-stroke-and-fill-
//! styling`, criteria 5, 6, 13, 14, 24, 36 to 39): the Style section's reads and
//! commands, plus the two calls the panel's shell needs (the view origin on a
//! toggle and the pointer state for its shortcut). A `impl WasmSession` block
//! of its own, so `wasm_api.rs` does not grow. Strings and scalars only (ADR
//! 0001 §5); every method is a direct pass-through to `Session`, which holds the
//! orchestration, over `curvyo-ui-core`'s rules.

use curvyo_document_core::Color;
use curvyo_ui_core::{
    DashChoice, Grid, MarkerSlot, PaintTarget, StyleField, ValueField, cap_from_name, hsv_to_rgb,
    join_from_name, marker_place_from_name, marker_shape_from_name, rgb_to_hsv,
};
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
    /// the code `hex`, `percent`, `width` or `dash`: the field stays open and
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

    /// A tick of a drag in the colour area or the hue slider: shows the colour
    /// of hue `hue` (degrees), saturation and value (`0` to `1`) on the edited
    /// objects without writing. `field_name` is `"stroke-color"` or
    /// `"fill-color"`.
    pub fn preview_style_hsv(&mut self, field_name: &str, hue: f64, saturation: f64, value: f64) {
        if let Some(field) = field(field_name) {
            self.session
                .preview_style_hsv(field, hue, saturation, value);
        }
    }

    /// A tick of a value field drag: position `p` (`0` to `1`) on the field's
    /// scale, rounded to `grid` (`"normal"`, `"coarse"` or `"fine"`), shown
    /// without writing. `field_name` is `"stroke-width"`, `"stroke-opacity"` or
    /// `"fill-opacity"`.
    pub fn preview_value_field(&mut self, field_name: &str, p: f64, grid_name: &str) {
        if let (Some(field), Some(grid)) = (
            ValueField::from_name(field_name),
            Grid::from_name(grid_name),
        ) {
            self.session.preview_value_field(field, p, grid);
        }
    }

    /// An arrow key on a value field: `steps` steps from the shown value,
    /// previewed; the key-up commits.
    pub fn step_value_field(&mut self, field_name: &str, steps: i32, grid_name: &str) {
        if let (Some(field), Some(grid)) = (
            ValueField::from_name(field_name),
            Grid::from_name(grid_name),
        ) {
            self.session.step_value_field(field, steps, grid);
        }
    }

    /// The reset icon or `Ctrl+Backspace`: the field's default for every edited
    /// object, one commit (none when the value already is the default). `false`
    /// when there is nothing to edit.
    pub fn reset_value_field(&mut self, field_name: &str) -> bool {
        ValueField::from_name(field_name).is_some_and(|field| self.session.reset_value_field(field))
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

    /// A Dash preset button: `"solid"`, `"dash"`, `"dot"` or `"dash-dot"`; any
    /// other word writes nothing.
    pub fn set_stroke_dash(&mut self, name: &str) {
        if let Some(choice) = DashChoice::from_name(name) {
            self.session.set_stroke_dash(choice);
        }
    }

    /// Enter or Tab in the pattern line: `"committed"`, `"unchanged"` (nothing
    /// to edit) or `"invalid:dash"`.
    pub fn set_stroke_dash_text(&mut self, text: &str) -> String {
        match self.session.set_stroke_dash_text(text) {
            Ok(true) => "committed".to_string(),
            Ok(false) => "unchanged".to_string(),
            Err(error) => format!("invalid:{}", error.code()),
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

    /// The fill Paint switch: one commit.
    pub fn set_fill_paint(&mut self, on: bool) {
        self.session.set_fill_paint(on);
    }

    /// A marker slot choice: `slot_name` is `"start"`, `"mid"` or `"end"`,
    /// `shape_name` `"none"`, `"arrow"` or `"dot"`. One commit to the paths of
    /// the edited objects.
    pub fn set_marker_shape(&mut self, slot_name: &str, shape_name: &str) {
        if let (Some(slot), Some(shape)) = (
            MarkerSlot::from_name(slot_name),
            marker_shape_from_name(shape_name),
        ) {
            self.session.set_marker_shape(slot, shape);
        }
    }

    /// The Place group: `"spaced"` or `"nodes"`.
    pub fn set_marker_place(&mut self, name: &str) {
        if let Some(place) = marker_place_from_name(name) {
            self.session.set_marker_place(place);
        }
    }

    /// The eyedropper button: starts picking a colour from the drawing for
    /// `"stroke"` or `"fill"`; pressing it again ends picking.
    pub fn begin_colour_pick(&mut self, target_name: &str) {
        if let Some(target) = PaintTarget::from_name(target_name) {
            self.session.begin_colour_pick(target);
        }
    }

    /// Ends picking and writes nothing (Escape, a press in the panel, a right
    /// press on the canvas).
    pub fn end_colour_pick(&mut self) {
        self.session.end_colour_pick();
    }

    /// What a click at the pointer would take while picking: empty, or the
    /// colour as `#RRGGBBAA` and the paint it comes from (`"stroke"` or
    /// `"fill"`). Call after `pointer_hover`.
    #[must_use]
    pub fn colour_pick_hover(&self) -> Vec<String> {
        self.session
            .colour_pick_hover()
            .map(|(hex, paint)| vec![hex, paint.name().to_string()])
            .unwrap_or_default()
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

    /// The hue (degrees, `-1` for a grey or black, which has none), saturation
    /// and value (`0` to `1`) of the colour `rgb` (`0xRRGGBB`): the picker
    /// re-derives its state with this when the colour changed from outside it.
    #[must_use]
    pub fn colour_hsv(&self, rgb: u32) -> Vec<f64> {
        let hsv = rgb_to_hsv(Color {
            r: ((rgb >> 16) & 0xFF) as u8,
            g: ((rgb >> 8) & 0xFF) as u8,
            b: (rgb & 0xFF) as u8,
        });
        vec![hsv.hue.unwrap_or(-1.0), hsv.saturation, hsv.value]
    }

    /// The colour `0xRRGGBB` of hue `hue` (degrees), saturation and value
    /// (`0` to `1`), rounded to the stored 8 bits.
    #[must_use]
    pub fn hsv_colour(&self, hue: f64, saturation: f64, value: f64) -> u32 {
        let color = hsv_to_rgb(hue, saturation, value);
        u32::from(color.r) << 16 | u32::from(color.g) << 8 | u32::from(color.b)
    }
}
