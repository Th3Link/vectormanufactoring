//! The Loro keys, absent defaults, reads and writes of the style schema
//! (`specs/0007-stroke-and-fill-styling/adrs.md`, "the style schema"), shared
//! by paths and primitives; open-file validation is `style_validation`.
//!
//! On-disk/in-CRDT shape, keys of an object's meta map, each its own LWW
//! register unless noted. An absent key reads as the frozen default:
//!
//! ```text
//!   stroke_enabled : bool                          absent = true
//!   stroke_width   : f64 mm, > 0                   absent = 0.25
//!   stroke         : [r, g, b]                     absent = black
//!   stroke_opacity : f64 in [0, 1]                 absent = 1
//!   stroke_dash    : [f64 ...] multiples of width  absent = [] (solid)
//!   stroke_join    : "miter" | "round" | "bevel"   absent = "miter"
//!   stroke_cap     : "butt" | "round" | "square"   absent = "butt"
//!   stroke_marker_start     : "none" | "arrow" | "dot"  absent = "none"
//!   stroke_marker_mid       : "none" | "arrow" | "dot"  absent = "none"
//!   stroke_marker_end       : "none" | "arrow" | "dot"  absent = "none"
//!   stroke_marker_mid_place : "spaced" | "nodes"        absent = "spaced"
//!   stroke_marker_mid_count : integer >= 1              absent = 1
//!   fill_enabled   : bool                          absent = false
//!   fill           : [r, g, b]                     absent = black
//!   fill_opacity   : f64 in [0, 1]                 absent = 1
//! ```
//!
//! A version-7 file written by a build with gradients may also hold `fill_kind`
//! and `fill_stops`; [`crate::legacy_fill`] reads past and drops them.

use loro::{LoroMap, LoroValue};

use crate::legacy_fill;
use crate::path_codec::{as_f64, as_u8};
use crate::path_model::Color;
use crate::style_model::{
    DashPattern, Fill, LineCap, LineJoin, MarkerCount, MarkerPlace, MarkerShape, Markers, Opacity,
    Stroke, Style,
};
use crate::units::Length;

pub(crate) const KEY_STROKE_ENABLED: &str = "stroke_enabled";
pub(crate) const KEY_STROKE_WIDTH: &str = "stroke_width";
pub(crate) const KEY_STROKE: &str = "stroke";
pub(crate) const KEY_STROKE_OPACITY: &str = "stroke_opacity";
pub(crate) const KEY_STROKE_DASH: &str = "stroke_dash";
pub(crate) const KEY_STROKE_JOIN: &str = "stroke_join";
pub(crate) const KEY_STROKE_CAP: &str = "stroke_cap";
pub(crate) const KEY_MARKER_START: &str = "stroke_marker_start";
pub(crate) const KEY_MARKER_MID: &str = "stroke_marker_mid";
pub(crate) const KEY_MARKER_END: &str = "stroke_marker_end";
pub(crate) const KEY_MARKER_MID_PLACE: &str = "stroke_marker_mid_place";
pub(crate) const KEY_MARKER_MID_COUNT: &str = "stroke_marker_mid_count";
pub(crate) const KEY_FILL_ENABLED: &str = "fill_enabled";
pub(crate) const KEY_FILL: &str = "fill";
pub(crate) const KEY_FILL_OPACITY: &str = "fill_opacity";

pub(crate) const JOIN_MITER: &str = "miter";
pub(crate) const JOIN_ROUND: &str = "round";
pub(crate) const JOIN_BEVEL: &str = "bevel";
pub(crate) const CAP_BUTT: &str = "butt";
pub(crate) const CAP_ROUND: &str = "round";
pub(crate) const CAP_SQUARE: &str = "square";
pub(crate) const SHAPE_NONE: &str = "none";
pub(crate) const SHAPE_ARROW: &str = "arrow";
pub(crate) const SHAPE_DOT: &str = "dot";
pub(crate) const PLACE_SPACED: &str = "spaced";
pub(crate) const PLACE_NODES: &str = "nodes";

/// Every style key of an object's meta map, none of them a shape-parameter
/// key: "object to path" strips `shape_codec::ALL_PRIMITIVE_KEYS` only, so
/// the style carries over to the converted path unchanged.
#[cfg(test)]
pub(crate) const ALL_STYLE_KEYS: &[&str] = &[
    KEY_STROKE_ENABLED,
    KEY_STROKE_WIDTH,
    KEY_STROKE,
    KEY_STROKE_OPACITY,
    KEY_STROKE_DASH,
    KEY_STROKE_JOIN,
    KEY_STROKE_CAP,
    KEY_MARKER_START,
    KEY_MARKER_MID,
    KEY_MARKER_END,
    KEY_MARKER_MID_PLACE,
    KEY_MARKER_MID_COUNT,
    KEY_FILL_ENABLED,
    KEY_FILL,
    KEY_FILL_OPACITY,
];

// ---------------------------------------------------------------------------
// Reads (lenient: a missing or odd value reads as its default, so a document
// merged from a peer never makes a read panic; open-file validation is the
// strict check)
// ---------------------------------------------------------------------------

pub(crate) fn read_value(meta: &LoroMap, key: &str) -> Option<LoroValue> {
    meta.get(key).map(|v| v.get_deep_value())
}

fn read_bool(meta: &LoroMap, key: &str, default: bool) -> bool {
    match read_value(meta, key) {
        Some(LoroValue::Bool(b)) => b,
        _ => default,
    }
}

fn read_number(meta: &LoroMap, key: &str) -> Option<f64> {
    read_value(meta, key)
        .and_then(|v| as_f64(&v))
        .filter(|n| n.is_finite())
}

fn read_string(meta: &LoroMap, key: &str) -> Option<String> {
    match read_value(meta, key) {
        Some(LoroValue::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

fn color_from_value(value: &LoroValue) -> Option<Color> {
    match value {
        LoroValue::List(list) if list.len() == 3 => Some(Color {
            r: as_u8(&list[0]),
            g: as_u8(&list[1]),
            b: as_u8(&list[2]),
        }),
        _ => None,
    }
}

fn read_color(meta: &LoroMap, key: &str) -> Option<Color> {
    color_from_value(&read_value(meta, key)?)
}

fn read_opacity(meta: &LoroMap, key: &str) -> Opacity {
    read_number(meta, key)
        .and_then(|n| Opacity::new(n).ok())
        .unwrap_or(Opacity::OPAQUE)
}

fn read_dash(meta: &LoroMap) -> DashPattern {
    match read_value(meta, KEY_STROKE_DASH) {
        Some(LoroValue::List(list)) => list
            .iter()
            .map(as_f64)
            .collect::<Option<Vec<f64>>>()
            .and_then(|lengths| DashPattern::new(lengths).ok())
            .unwrap_or_default(),
        _ => DashPattern::solid(),
    }
}

fn read_join(meta: &LoroMap) -> LineJoin {
    match read_string(meta, KEY_STROKE_JOIN).as_deref() {
        Some(JOIN_ROUND) => LineJoin::Round,
        Some(JOIN_BEVEL) => LineJoin::Bevel,
        _ => LineJoin::Miter,
    }
}

fn read_marker_shape(meta: &LoroMap, key: &str) -> MarkerShape {
    match read_string(meta, key).as_deref() {
        Some(SHAPE_ARROW) => MarkerShape::Arrow,
        Some(SHAPE_DOT) => MarkerShape::Dot,
        _ => MarkerShape::None,
    }
}

fn read_markers(meta: &LoroMap) -> Markers {
    Markers {
        start: read_marker_shape(meta, KEY_MARKER_START),
        mid: read_marker_shape(meta, KEY_MARKER_MID),
        end: read_marker_shape(meta, KEY_MARKER_END),
        mid_place: match read_string(meta, KEY_MARKER_MID_PLACE).as_deref() {
            Some(PLACE_NODES) => MarkerPlace::AtNodes,
            _ => MarkerPlace::Spaced,
        },
        // A count below 1 or not a whole number reads as 1 (lenient: only a
        // merged document can hold one; open validation refuses a file's).
        mid_count: match read_value(meta, KEY_MARKER_MID_COUNT) {
            // A count above `u32::MAX` saturates (criterion 29 draws 500).
            Some(LoroValue::I64(n)) if n >= 1 => {
                MarkerCount::new(u32::try_from(n).unwrap_or(u32::MAX)).unwrap_or(MarkerCount::ONE)
            }
            _ => MarkerCount::ONE,
        },
    }
}

fn read_cap(meta: &LoroMap) -> LineCap {
    match read_string(meta, KEY_STROKE_CAP).as_deref() {
        Some(CAP_ROUND) => LineCap::Round,
        Some(CAP_SQUARE) => LineCap::Square,
        _ => LineCap::Butt,
    }
}

pub(crate) fn read_width(meta: &LoroMap) -> Length {
    read_number(meta, KEY_STROKE_WIDTH)
        .filter(|mm| *mm > 0.0)
        .map_or_else(|| Style::default().stroke.width, Length::from_mm)
}

/// Reads an object's whole style, every absent key at its frozen default.
pub(crate) fn read_style(meta: &LoroMap) -> Style {
    let default = Style::default();
    Style {
        stroke: Stroke {
            enabled: read_bool(meta, KEY_STROKE_ENABLED, default.stroke.enabled),
            width: read_width(meta),
            color: read_color(meta, KEY_STROKE).unwrap_or(default.stroke.color),
            opacity: read_opacity(meta, KEY_STROKE_OPACITY),
            dash: read_dash(meta),
            join: read_join(meta),
            cap: read_cap(meta),
            markers: read_markers(meta),
        },
        fill: Fill {
            enabled: read_bool(meta, KEY_FILL_ENABLED, default.fill.enabled)
                && legacy_fill::reads_as_solid(meta),
            color: read_color(meta, KEY_FILL).unwrap_or(default.fill.color),
            opacity: read_opacity(meta, KEY_FILL_OPACITY),
        },
    }
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

pub(crate) fn color_to_value(color: Color) -> Vec<i64> {
    vec![i64::from(color.r), i64::from(color.g), i64::from(color.b)]
}

/// Inserts a plain value under a known key.
fn insert(meta: &LoroMap, key: &str, value: impl Into<LoroValue>) {
    // invariant: inserting a plain value under a known key on an attached map
    // cannot fail.
    #[allow(clippy::unwrap_used)]
    meta.insert(key, value).unwrap();
}

fn write_bool(meta: &LoroMap, key: &str, value: bool) {
    insert(meta, key, value);
}

fn write_number(meta: &LoroMap, key: &str, value: f64) {
    insert(meta, key, value);
}

fn write_text(meta: &LoroMap, key: &str, value: &str) {
    insert(meta, key, value);
}

fn write_color(meta: &LoroMap, key: &str, color: Color) {
    insert(meta, key, color_to_value(color));
}

/// Writes the width of a stroke whose width is already known valid (greater
/// than zero and finite); the resize commands and the style edits both call
/// this.
pub(crate) fn write_stroke_width(meta: &LoroMap, width: Length) {
    write_number(meta, KEY_STROKE_WIDTH, width.as_mm());
}

/// Whether a resize's optional width may be written: absent, or a finite
/// number above zero (the open-file rule for `stroke_width`).
pub(crate) fn stroke_width_is_writable(width: Option<Length>) -> bool {
    width.is_none_or(|w| w.as_mm().is_finite() && w.as_mm() > 0.0)
}

const fn join_text(join: LineJoin) -> &'static str {
    match join {
        LineJoin::Miter => JOIN_MITER,
        LineJoin::Round => JOIN_ROUND,
        LineJoin::Bevel => JOIN_BEVEL,
    }
}

const fn cap_text(cap: LineCap) -> &'static str {
    match cap {
        LineCap::Butt => CAP_BUTT,
        LineCap::Round => CAP_ROUND,
        LineCap::Square => CAP_SQUARE,
    }
}

const fn shape_text(shape: MarkerShape) -> &'static str {
    match shape {
        MarkerShape::None => SHAPE_NONE,
        MarkerShape::Arrow => SHAPE_ARROW,
        MarkerShape::Dot => SHAPE_DOT,
    }
}

/// Writes the marker slots of `new` that differ from `old`; returns whether
/// anything was written.
fn write_marker_changes(meta: &LoroMap, old: Markers, new: Markers) -> bool {
    let mut wrote = false;
    for (key, before, after) in [
        (KEY_MARKER_START, old.start, new.start),
        (KEY_MARKER_MID, old.mid, new.mid),
        (KEY_MARKER_END, old.end, new.end),
    ] {
        if before != after {
            write_text(meta, key, shape_text(after));
            wrote = true;
        }
    }
    if old.mid_place != new.mid_place {
        let text = match new.mid_place {
            MarkerPlace::Spaced => PLACE_SPACED,
            MarkerPlace::AtNodes => PLACE_NODES,
        };
        write_text(meta, KEY_MARKER_MID_PLACE, text);
        wrote = true;
    }
    if old.mid_count != new.mid_count {
        insert(meta, KEY_MARKER_MID_COUNT, i64::from(new.mid_count.get()));
        wrote = true;
    }
    wrote
}

/// Writes what a brand-new object carries: the width and colour, explicitly,
/// and nothing else. Every other key is absent and reads as its default.
pub(crate) fn write_creation_style(meta: &LoroMap) {
    let default = Style::default();
    write_stroke_width(meta, default.stroke.width);
    write_color(meta, KEY_STROKE, default.stroke.color);
}

/// Writes every scalar key of `new` that differs from `old` and returns
/// whether anything was written. A value equal to the stored one is never
/// rewritten: an LWW rewrite of an unchanged value is a new operation that
/// could win against a peer's real edit (`adrs.md`, register granularity 5).
pub(crate) fn write_changes(meta: &LoroMap, old: &Style, new: &Style) -> bool {
    let (os, ns) = (&old.stroke, &new.stroke);
    let (of, nf) = (&old.fill, &new.fill);
    let mut wrote = false;
    if of != nf {
        // Any fill write replaces a legacy gradient fill (`legacy_fill`).
        legacy_fill::forget(meta);
    }
    if os.enabled != ns.enabled {
        write_bool(meta, KEY_STROKE_ENABLED, ns.enabled);
        wrote = true;
    }
    if os.width != ns.width {
        write_stroke_width(meta, ns.width);
        wrote = true;
    }
    if os.color != ns.color {
        write_color(meta, KEY_STROKE, ns.color);
        wrote = true;
    }
    if os.opacity != ns.opacity {
        write_number(meta, KEY_STROKE_OPACITY, ns.opacity.get());
        wrote = true;
    }
    if os.dash != ns.dash {
        write_dash(meta, &ns.dash);
        wrote = true;
    }
    if os.join != ns.join {
        write_text(meta, KEY_STROKE_JOIN, join_text(ns.join));
        wrote = true;
    }
    if os.cap != ns.cap {
        write_text(meta, KEY_STROKE_CAP, cap_text(ns.cap));
        wrote = true;
    }
    wrote |= write_marker_changes(meta, os.markers, ns.markers);
    if of.enabled != nf.enabled {
        write_bool(meta, KEY_FILL_ENABLED, nf.enabled);
        wrote = true;
    }
    if of.color != nf.color {
        write_color(meta, KEY_FILL, nf.color);
        wrote = true;
    }
    if of.opacity != nf.opacity {
        write_number(meta, KEY_FILL_OPACITY, nf.opacity.get());
        wrote = true;
    }
    wrote
}

fn write_dash(meta: &LoroMap, dash: &DashPattern) {
    insert(meta, KEY_STROKE_DASH, dash.as_slice().to_vec());
}

/// Writes a whole style onto a freshly created meta map (Split's new object):
/// the width and colour always, every other scalar key only where it differs
/// from its default.
pub(crate) fn write_style(meta: &LoroMap, style: &Style) {
    write_stroke_width(meta, style.stroke.width);
    write_color(meta, KEY_STROKE, style.stroke.color);
    let mut baseline = Style::default();
    baseline.stroke.width = style.stroke.width;
    baseline.stroke.color = style.stroke.color;
    write_changes(meta, &baseline, style);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Object to path" deletes `ALL_PRIMITIVE_KEYS` and nothing else, so a
    /// style key in that list would silently drop the style on conversion
    /// (acceptance criterion 33).
    #[test]
    fn no_style_key_is_a_primitive_key() {
        for key in ALL_STYLE_KEYS {
            assert!(
                !crate::shape_codec::ALL_PRIMITIVE_KEYS.contains(key),
                "{key} would be stripped by `object to path`"
            );
        }
    }

    #[test]
    fn a_fresh_object_carries_only_the_width_and_the_colour() {
        let document = crate::Document::new(1);
        let id = document.create_path(&[], false);
        let tree = document.loro().get_tree(crate::document::OBJECTS_TREE);
        let meta = tree.get_meta(crate::paths::tree_id_of(id)).unwrap();
        let written: Vec<String> = ALL_STYLE_KEYS
            .iter()
            .filter(|key| meta.get(key).is_some())
            .map(|key| (*key).to_string())
            .collect();
        assert_eq!(written, [KEY_STROKE_WIDTH, KEY_STROKE]);
    }
}
