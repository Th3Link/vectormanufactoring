//! The Style panel's state as the flat, scalar-and-string record the host
//! reads (ADR 0001 §5): one read after every change, no editing logic. Kept
//! apart from `style.rs` so the conversion is plain Rust a host test can pin.

use curvyo_document_core::{Color, Length, LineCap, LineJoin, Style};
use curvyo_ui_core::{
    BarValue, DashChoice, DashShown, Rgba, StylePanelState, ValueScale, hex_text,
};

/// What the Style panel shows. A `*_mixed` flag means the edited objects
/// differ (the field is empty with the placeholder "Mixed"); the value beside
/// it is then meaningless. Colours are `0xRRGGBB`, opacities whole-or-not
/// percents (the host rounds for display), words are the lower-case names the
/// host sends back. `*_rows` says whether the rows under a Paint switch are
/// shown (they are not while every edited paint is off).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent shown/mixed flags, read by name
pub struct StylePanelView {
    /// The subject line.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub subject: String,
    /// Changes whenever the edited objects change, so the host can drop state
    /// that belongs to other objects.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub scope_key: String,
    /// `"on"`, `"off"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_paint: String,
    /// The stroke rows under the Paint switch are shown.
    pub stroke_rows: bool,
    /// The stroke colours differ in red, green or blue.
    pub stroke_color_mixed: bool,
    /// The stroke colour.
    pub stroke_color: u32,
    /// The stroke colour and alpha as `#RRGGBBAA`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_hex: String,
    /// The stroke colour or alpha differ.
    pub stroke_hex_mixed: bool,
    /// The stroke opacities differ.
    pub stroke_opacity_mixed: bool,
    /// The stroke opacity, percent.
    pub stroke_opacity: f64,
    /// The stroke widths differ.
    pub stroke_width_mixed: bool,
    /// The stroke width, millimetres.
    pub stroke_width: f64,
    /// The width as the field shows it (up to three decimals).
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_width_text: String,
    /// The share of the field's width the bar is filled to (`0` to `1`).
    pub stroke_width_bar: f64,
    /// The value differs from the default or is mixed: the reset icon shows.
    pub stroke_width_resettable: bool,
    /// The stroke opacity as the field shows it.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_opacity_text: String,
    /// The bar position of the stroke opacity.
    pub stroke_opacity_bar: f64,
    /// The reset icon of the stroke opacity shows.
    pub stroke_opacity_resettable: bool,
    /// The pressed preset: `"solid"`, `"dash"`, `"dot"`, `"dash-dot"`,
    /// `"none"` (a list that is no preset) or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_dash: String,
    /// The dash list as the text line shows it; empty for solid and mixed.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_dash_text: String,
    /// `"miter"`, `"round"`, `"bevel"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_join: String,
    /// `"butt"`, `"round"`, `"square"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_cap: String,
    /// `"on"`, `"off"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub fill_paint: String,
    /// The fill rows under the Paint switch are shown.
    pub fill_rows: bool,
    /// The fill colours differ in red, green or blue.
    pub fill_color_mixed: bool,
    /// The fill colour.
    pub fill_color: u32,
    /// The fill colour and alpha as `#RRGGBBAA`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub fill_hex: String,
    /// The fill colour or alpha differ.
    pub fill_hex_mixed: bool,
    /// The fill opacities differ.
    pub fill_opacity_mixed: bool,
    /// The fill opacity, percent.
    pub fill_opacity: f64,
    /// The fill opacity as the field shows it.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub fill_opacity_text: String,
    /// The bar position of the fill opacity.
    pub fill_opacity_bar: f64,
    /// The reset icon of the fill opacity shows.
    pub fill_opacity_resettable: bool,
}

fn word<T: Copy>(value: BarValue<T>, name: impl Fn(T) -> &'static str) -> String {
    match value {
        BarValue::Uniform(v) => name(v).to_string(),
        BarValue::Mixed => "mixed".to_string(),
    }
}

fn pack_rgb(color: Color) -> u32 {
    u32::from(color.r) << 16 | u32::from(color.g) << 8 | u32::from(color.b)
}

fn map_value<T: Copy>(value: BarValue<T>, convert: impl Fn(T) -> f64) -> BarValue<f64> {
    match value {
        BarValue::Uniform(v) => BarValue::Uniform(convert(v)),
        BarValue::Mixed => BarValue::Mixed,
    }
}

/// `(mixed, value)` of a colour, `0` when mixed.
fn colour(value: BarValue<Color>) -> (bool, u32) {
    match value {
        BarValue::Uniform(color) => (false, pack_rgb(color)),
        BarValue::Mixed => (true, 0),
    }
}

/// `(mixed, percent)` of an opacity, `100` when mixed.
fn percent(value: BarValue<curvyo_document_core::Opacity>) -> (bool, f64) {
    match value {
        BarValue::Uniform(opacity) => (false, opacity.get() * 100.0),
        BarValue::Mixed => (true, 100.0),
    }
}

/// `(mixed, text)` of a colour with its alpha.
fn hex(value: BarValue<Rgba>) -> (bool, String) {
    match value {
        BarValue::Uniform((color, opacity)) => (false, hex_text(color, opacity)),
        BarValue::Mixed => (true, String::new()),
    }
}

/// `(text, bar, resettable)` of a value field: the text it shows, the share of
/// its width the bar fills, and whether the reset icon shows (the value is
/// mixed or not the default).
fn value_field(scale: ValueScale, value: BarValue<f64>) -> (String, f64, bool) {
    match value {
        BarValue::Uniform(v) => (
            scale.text(v),
            scale.position_of(v),
            (v - scale.default_value()).abs() > 1e-9,
        ),
        BarValue::Mixed => (String::new(), 0.0, true),
    }
}

impl StylePanelView {
    /// The record for `state`, tagged with the key of the edited objects.
    #[must_use]
    pub fn new(state: &StylePanelState, scope_key: String) -> Self {
        let (stroke_color_mixed, stroke_color) = colour(state.stroke.color);
        let (stroke_hex_mixed, stroke_hex) = hex(state.stroke.rgba);
        let (stroke_opacity_mixed, stroke_opacity) = percent(state.stroke.opacity);
        let (fill_color_mixed, fill_color) = colour(state.fill.color);
        let (fill_hex_mixed, fill_hex) = hex(state.fill.rgba);
        let (fill_opacity_mixed, fill_opacity) = percent(state.fill.opacity);
        let (stroke_width_text, stroke_width_bar, stroke_width_resettable) = value_field(
            ValueScale::StrokeWidth,
            map_value(state.stroke.width, Length::as_mm),
        );
        let (stroke_opacity_text, stroke_opacity_bar, stroke_opacity_resettable) = value_field(
            ValueScale::Opacity,
            map_value(state.stroke.opacity, |o| o.get() * 100.0),
        );
        let (fill_opacity_text, fill_opacity_bar, fill_opacity_resettable) = value_field(
            ValueScale::Opacity,
            map_value(state.fill.opacity, |o| o.get() * 100.0),
        );
        let (stroke_width_mixed, stroke_width) = match state.stroke.width {
            BarValue::Uniform(width) => (false, width.as_mm()),
            BarValue::Mixed => (true, 0.0),
        };
        let (stroke_dash, stroke_dash_text) = match &state.stroke.dash {
            DashShown::Uniform { preset, text } => (
                match preset {
                    Some(DashChoice::Solid) => "solid",
                    Some(DashChoice::Dash) => "dash",
                    Some(DashChoice::Dot) => "dot",
                    Some(DashChoice::DashDot) => "dash-dot",
                    None => "none",
                }
                .to_string(),
                text.clone(),
            ),
            DashShown::Mixed => ("mixed".to_string(), String::new()),
        };
        Self {
            subject: state.subject.clone(),
            scope_key,
            stroke_paint: word(state.stroke.paint, |on| if on { "on" } else { "off" }),
            stroke_rows: state.stroke.rows_shown,
            stroke_color_mixed,
            stroke_color,
            stroke_hex,
            stroke_hex_mixed,
            stroke_opacity_mixed,
            stroke_opacity,
            stroke_width_mixed,
            stroke_width,
            stroke_width_text,
            stroke_width_bar,
            stroke_width_resettable,
            stroke_opacity_text,
            stroke_opacity_bar,
            stroke_opacity_resettable,
            stroke_dash,
            stroke_dash_text,
            stroke_join: word(state.stroke.join, |join| match join {
                LineJoin::Miter => "miter",
                LineJoin::Round => "round",
                LineJoin::Bevel => "bevel",
            }),
            stroke_cap: word(state.stroke.cap, |cap| match cap {
                LineCap::Butt => "butt",
                LineCap::Round => "round",
                LineCap::Square => "square",
            }),
            fill_paint: word(state.fill.paint, |on| if on { "on" } else { "off" }),
            fill_rows: state.fill.rows_shown,
            fill_color_mixed,
            fill_color,
            fill_hex,
            fill_hex_mixed,
            fill_opacity_mixed,
            fill_opacity,
            fill_opacity_text,
            fill_opacity_bar,
            fill_opacity_resettable,
        }
    }

    /// Shows `width` (millimetres) in the Width field, for a drag in flight.
    pub fn show_width(&mut self, width: f64) {
        self.stroke_width = width;
        self.stroke_width_mixed = false;
        self.stroke_width_text = ValueScale::StrokeWidth.text(width);
        self.stroke_width_bar = ValueScale::StrokeWidth.position_of(width);
        self.stroke_width_resettable =
            (width - ValueScale::StrokeWidth.default_value()).abs() > 1e-9;
    }

    /// The record while there is nothing to edit: the host renders no Style
    /// area then, so only the defaults of the fields matter.
    #[must_use]
    pub fn empty() -> Self {
        let defaults = Style::default();
        Self {
            subject: String::new(),
            scope_key: String::new(),
            stroke_paint: "on".to_string(),
            stroke_rows: true,
            stroke_color_mixed: false,
            stroke_color: pack_rgb(defaults.stroke.color),
            stroke_hex: hex_text(defaults.stroke.color, defaults.stroke.opacity),
            stroke_hex_mixed: false,
            stroke_opacity_mixed: false,
            stroke_opacity: defaults.stroke.opacity.get() * 100.0,
            stroke_width_mixed: false,
            stroke_width: defaults.stroke.width.as_mm(),
            stroke_width_text: ValueScale::StrokeWidth.text(defaults.stroke.width.as_mm()),
            stroke_width_bar: ValueScale::StrokeWidth.position_of(defaults.stroke.width.as_mm()),
            stroke_width_resettable: false,
            stroke_opacity_text: "100".to_string(),
            stroke_opacity_bar: 1.0,
            stroke_opacity_resettable: false,
            stroke_dash: "solid".to_string(),
            stroke_dash_text: String::new(),
            stroke_join: "miter".to_string(),
            stroke_cap: "butt".to_string(),
            fill_paint: "off".to_string(),
            fill_rows: false,
            fill_color_mixed: false,
            fill_color: pack_rgb(defaults.fill.color),
            fill_hex: hex_text(defaults.fill.color, defaults.fill.opacity),
            fill_hex_mixed: false,
            fill_opacity_mixed: false,
            fill_opacity: defaults.fill.opacity.get() * 100.0,
            fill_opacity_text: "100".to_string(),
            fill_opacity_bar: 1.0,
            fill_opacity_resettable: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::session::{Session, Tool};
    use curvyo_document_core::Point;

    fn session_with_a_rectangle() -> Session {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);
        session
    }

    /// The default style reads as the frozen defaults, in the host's words.
    #[test]
    fn the_view_names_the_defaults() {
        let session = session_with_a_rectangle();
        let view = session.style_panel_view();
        assert_eq!(view.subject, "Rectangle");
        assert_eq!(view.stroke_hex, "#000000FF");
        assert!(view.stroke_rows && !view.fill_rows);
        assert_eq!(view.stroke_paint, "on");
        assert_eq!(view.stroke_color, 0);
        assert!((view.stroke_width - 0.25).abs() < 1e-9);
        assert!((view.stroke_opacity - 100.0).abs() < 1e-9);
        assert_eq!(view.stroke_dash, "solid");
        assert_eq!(view.stroke_join, "miter");
        assert_eq!(view.stroke_cap, "butt");
        assert_eq!(view.fill_paint, "off");
        assert!(!view.stroke_color_mixed && !view.fill_opacity_mixed);
    }

    /// A colour packs as 0xRRGGBB and the scope key follows the selection.
    #[test]
    fn a_colour_packs_and_the_scope_key_follows_the_selection() {
        let mut session = session_with_a_rectangle();
        let key = session.style_panel_view().scope_key;
        assert_eq!(
            session.set_style_text(curvyo_ui_core::StyleField::FillColor, "#2F6FEE"),
            Ok(true)
        );
        let view = session.style_panel_view();
        assert_eq!(view.fill_color, 0x002F_6FEE);
        assert_eq!(view.fill_hex, "#2F6FEEFF");
        assert_eq!(view.scope_key, key, "same objects, same key");
        session.set_tool(Tool::Rectangle);
        assert_ne!(session.style_panel_view().scope_key, key);
    }
}
