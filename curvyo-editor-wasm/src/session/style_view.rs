//! The Style panel's state as the flat, scalar-and-string record the host
//! reads (ADR 0001 §5): one read after every change, no editing logic. Kept
//! apart from `style.rs` so the conversion is plain Rust a host test can pin.

use curvyo_document_core::{Color, FillMode, LineCap, LineJoin};
use curvyo_ui_core::{BarValue, DashChoice, StopsPanel, StylePanelState};

/// What the Style panel shows. A `*_mixed` flag means the edited objects
/// differ (the field is empty with the placeholder "Mixed"); the value beside
/// it is then meaningless. Colours are `0xRRGGBB`, opacities whole-or-not
/// percents (the host rounds for display), words are the lower-case names the
/// host sends back.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent shown/enabled/mixed flags, read by name
pub struct StylePanelView {
    /// The subject line.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub subject: String,
    /// Changes whenever the edited objects change, so the host can close a
    /// colour popover that belongs to other objects.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub scope_key: String,
    /// There is something to edit; otherwise every control is disabled.
    pub enabled: bool,
    /// `"on"`, `"off"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_paint: String,
    /// Every edited stroke is off: Dash, Join and Cap are disabled.
    pub stroke_all_off: bool,
    /// The stroke colours differ.
    pub stroke_color_mixed: bool,
    /// The stroke colour.
    pub stroke_color: u32,
    /// The stroke opacities differ.
    pub stroke_opacity_mixed: bool,
    /// The stroke opacity, percent.
    pub stroke_opacity: f64,
    /// The stroke widths differ.
    pub stroke_width_mixed: bool,
    /// The stroke width, millimetres.
    pub stroke_width: f64,
    /// `"solid"`, `"dash"`, `"dot"`, `"dash-dot"`, `"custom"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_dash: String,
    /// `"miter"`, `"round"`, `"bevel"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_join: String,
    /// `"butt"`, `"round"`, `"square"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stroke_cap: String,
    /// `"none"`, `"solid"`, `"linear"`, `"radial"` or `"mixed"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub fill_mode: String,
    /// The fill colours differ.
    pub fill_color_mixed: bool,
    /// The solid fill colour.
    pub fill_color: u32,
    /// The fill opacities differ.
    pub fill_opacity_mixed: bool,
    /// The solid fill opacity, percent.
    pub fill_opacity: f64,
    /// The gradient stop editor: `"hidden"`, `"different-counts"` or
    /// `"editor"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stops_state: String,
    /// How many objects the stop editor edits.
    pub stops_objects: u32,
    /// Add stop and a bar click work.
    pub stops_can_add: bool,
    /// Remove works.
    pub stops_can_remove: bool,
    /// The selection holds a polygon or star: the gradient box note shows.
    pub stops_box_note: bool,
    /// The bar shows its ramp and thumbs (every list equal in value).
    pub stops_bar_shown: bool,
    /// One row of the stop list per rank, six numbers each: position percent,
    /// 1 if the positions differ, colour `0xRRGGBB`, 1 if the colours differ,
    /// opacity percent, 1 if the opacities differ.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stop_rows: Vec<f64>,
    /// The bar's stops in position order, three numbers each: position percent,
    /// colour `0xRRGGBB`, opacity percent; empty unless `stops_bar_shown`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub stop_bar: Vec<f64>,
    /// The selected stop's rank, or -1.
    pub selected_stop: i32,
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

/// The stop editor's fields of the view.
#[allow(clippy::struct_excessive_bools)] // independent flags, read by name
struct StopsFields {
    state: &'static str,
    objects: u32,
    can_add: bool,
    can_remove: bool,
    box_note: bool,
    bar_shown: bool,
    rows: Vec<f64>,
    bar: Vec<f64>,
}

fn flag(mixed: bool) -> f64 {
    f64::from(u8::from(mixed))
}

fn stops_fields(panel: &StopsPanel) -> StopsFields {
    let mut fields = StopsFields {
        state: "hidden",
        objects: 0,
        can_add: false,
        can_remove: false,
        box_note: false,
        bar_shown: false,
        rows: Vec::new(),
        bar: Vec::new(),
    };
    match panel {
        StopsPanel::Hidden => {}
        StopsPanel::DifferentCounts => fields.state = "different-counts",
        StopsPanel::Editor(view) => {
            fields.state = "editor";
            fields.objects = u32::try_from(view.objects).unwrap_or(u32::MAX);
            fields.can_add = view.can_add;
            fields.can_remove = view.can_remove;
            fields.box_note = view.box_note;
            for row in &view.rows {
                let (position_mixed, position) = match row.position {
                    BarValue::Uniform(position) => (false, position * 100.0),
                    BarValue::Mixed => (true, 0.0),
                };
                let (color_mixed, color) = colour(row.color);
                let (opacity_mixed, opacity) = percent(row.opacity);
                fields.rows.extend([
                    position,
                    flag(position_mixed),
                    f64::from(color),
                    flag(color_mixed),
                    opacity,
                    flag(opacity_mixed),
                ]);
            }
            if let Some(bar) = &view.bar {
                fields.bar_shown = true;
                for stop in bar {
                    fields.bar.extend([
                        stop.position * 100.0,
                        f64::from(pack_rgb(stop.color)),
                        stop.opacity.get() * 100.0,
                    ]);
                }
            }
        }
    }
    fields
}

impl StylePanelView {
    /// The record for `state`, tagged with the key of the edited objects and the
    /// rank of the selected stop.
    #[must_use]
    pub fn new(state: &StylePanelState, scope_key: String, selected_stop: Option<usize>) -> Self {
        let stops = stops_fields(&state.fill.stops);
        let (stroke_color_mixed, stroke_color) = colour(state.stroke.color);
        let (stroke_opacity_mixed, stroke_opacity) = percent(state.stroke.opacity);
        let (fill_color_mixed, fill_color) = colour(state.fill.color);
        let (fill_opacity_mixed, fill_opacity) = percent(state.fill.opacity);
        let (stroke_width_mixed, stroke_width) = match state.stroke.width {
            BarValue::Uniform(width) => (false, width.as_mm()),
            BarValue::Mixed => (true, 0.0),
        };
        Self {
            subject: state.subject.clone(),
            scope_key,
            enabled: state.enabled,
            stroke_paint: word(state.stroke.paint, |on| if on { "on" } else { "off" }),
            stroke_all_off: state.stroke.all_off,
            stroke_color_mixed,
            stroke_color,
            stroke_opacity_mixed,
            stroke_opacity,
            stroke_width_mixed,
            stroke_width,
            stroke_dash: word(state.stroke.dash, |dash| match dash {
                DashChoice::Solid => "solid",
                DashChoice::Dash => "dash",
                DashChoice::Dot => "dot",
                DashChoice::DashDot => "dash-dot",
                DashChoice::Custom => "custom",
            }),
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
            fill_mode: word(state.fill.mode, |mode| match mode {
                FillMode::None => "none",
                FillMode::Solid => "solid",
                FillMode::Linear => "linear",
                FillMode::Radial => "radial",
            }),
            fill_color_mixed,
            fill_color,
            fill_opacity_mixed,
            fill_opacity,
            stops_state: stops.state.to_string(),
            stops_objects: stops.objects,
            stops_can_add: stops.can_add,
            stops_can_remove: stops.can_remove,
            stops_box_note: stops.box_note,
            stops_bar_shown: stops.bar_shown,
            stop_rows: stops.rows,
            stop_bar: stops.bar,
            selected_stop: selected_stop
                .and_then(|rank| i32::try_from(rank).ok())
                .unwrap_or(-1),
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
        assert!(view.enabled);
        assert_eq!(view.stroke_paint, "on");
        assert_eq!(view.stroke_color, 0);
        assert!((view.stroke_width - 0.25).abs() < 1e-9);
        assert!((view.stroke_opacity - 100.0).abs() < 1e-9);
        assert_eq!(view.stroke_dash, "solid");
        assert_eq!(view.stroke_join, "miter");
        assert_eq!(view.stroke_cap, "butt");
        assert_eq!(view.fill_mode, "none");
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
        assert_eq!(view.scope_key, key, "same objects, same key");
        session.set_tool(Tool::Rectangle);
        assert_ne!(session.style_panel_view().scope_key, key);
    }
}
