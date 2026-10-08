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
//!   fill_enabled   : bool                          absent = false
//!   fill_kind      : "solid" | "linear" | "radial" absent = "solid"
//!   fill           : [r, g, b]                     absent = black
//!   fill_opacity   : f64 in [0, 1]                 absent = 1
//!   fill_stops     : movable list of stop maps     absent = no stops
//!     id           : hex string of the StopId      written once
//!     position     : f64 in [0, 1]                 LWW register
//!     color        : [r, g, b]                     LWW register
//!     opacity      : f64 in [0, 1]                 LWW register
//! ```

use loro::{Container, LoroMap, LoroMovableList, LoroValue, ValueOrContainer};

use crate::path_codec::{as_f64, as_u8};
use crate::path_model::Color;
use crate::style_model::{
    DashPattern, Fill, FillKind, GradientStop, LineCap, LineJoin, Opacity, StopId, StopPosition,
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
pub(crate) const KEY_FILL_ENABLED: &str = "fill_enabled";
pub(crate) const KEY_FILL_KIND: &str = "fill_kind";
pub(crate) const KEY_FILL: &str = "fill";
pub(crate) const KEY_FILL_OPACITY: &str = "fill_opacity";
pub(crate) const KEY_FILL_STOPS: &str = "fill_stops";

pub(crate) const KEY_STOP_ID: &str = "id";
pub(crate) const KEY_STOP_POSITION: &str = "position";
pub(crate) const KEY_STOP_COLOR: &str = "color";
pub(crate) const KEY_STOP_OPACITY: &str = "opacity";

pub(crate) const JOIN_MITER: &str = "miter";
pub(crate) const JOIN_ROUND: &str = "round";
pub(crate) const JOIN_BEVEL: &str = "bevel";
pub(crate) const CAP_BUTT: &str = "butt";
pub(crate) const CAP_ROUND: &str = "round";
pub(crate) const CAP_SQUARE: &str = "square";
pub(crate) const KIND_SOLID: &str = "solid";
pub(crate) const KIND_LINEAR: &str = "linear";
pub(crate) const KIND_RADIAL: &str = "radial";

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
    KEY_FILL_ENABLED,
    KEY_FILL_KIND,
    KEY_FILL,
    KEY_FILL_OPACITY,
    KEY_FILL_STOPS,
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

fn read_cap(meta: &LoroMap) -> LineCap {
    match read_string(meta, KEY_STROKE_CAP).as_deref() {
        Some(CAP_ROUND) => LineCap::Round,
        Some(CAP_SQUARE) => LineCap::Square,
        _ => LineCap::Butt,
    }
}

fn read_kind(meta: &LoroMap) -> FillKind {
    match read_string(meta, KEY_FILL_KIND).as_deref() {
        Some(KIND_LINEAR) => FillKind::Linear,
        Some(KIND_RADIAL) => FillKind::Radial,
        _ => FillKind::Solid,
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
        },
        fill: Fill {
            enabled: read_bool(meta, KEY_FILL_ENABLED, default.fill.enabled),
            kind: read_kind(meta),
            color: read_color(meta, KEY_FILL).unwrap_or(default.fill.color),
            opacity: read_opacity(meta, KEY_FILL_OPACITY),
            stops: read_stops(meta),
        },
    }
}

pub(crate) fn read_stop(map: &LoroMap) -> Option<GradientStop> {
    Some(GradientStop {
        id: read_stop_id(map)?,
        position: StopPosition::new(read_number(map, KEY_STOP_POSITION)?).ok()?,
        color: read_color(map, KEY_STOP_COLOR)?,
        opacity: Opacity::new(read_number(map, KEY_STOP_OPACITY)?).ok()?,
    })
}

fn read_stop_id(map: &LoroMap) -> Option<StopId> {
    read_string(map, KEY_STOP_ID).and_then(|hex| StopId::from_hex(&hex))
}

/// Reads the stop list in list order; a stop map a peer left incomplete is
/// skipped rather than failing the read.
fn read_stops(meta: &LoroMap) -> Vec<GradientStop> {
    let Some(list) = stops_list(meta) else {
        return Vec::new();
    };
    (0..list.len())
        .filter_map(|index| stop_map_at(&list, index))
        .filter_map(|map| read_stop(&map))
        .collect()
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

const fn kind_text(kind: FillKind) -> &'static str {
    match kind {
        FillKind::Solid => KIND_SOLID,
        FillKind::Linear => KIND_LINEAR,
        FillKind::Radial => KIND_RADIAL,
    }
}

/// Writes what a brand-new object carries: the width and colour, explicitly,
/// and nothing else. Every other key is absent and reads as its default.
pub(crate) fn write_creation_style(meta: &LoroMap) {
    let default = Style::default();
    write_stroke_width(meta, default.stroke.width);
    write_color(meta, KEY_STROKE, default.stroke.color);
}

/// Writes every scalar key of `new` that differs from `old` and returns
/// whether anything was written. The gradient stops are not touched here;
/// they have their own commands. A value equal to the stored one is never
/// rewritten: an LWW rewrite of an unchanged value is a new operation that
/// could win against a peer's real edit (`adrs.md`, register granularity 5).
pub(crate) fn write_changes(meta: &LoroMap, old: &Style, new: &Style) -> bool {
    let (os, ns) = (&old.stroke, &new.stroke);
    let (of, nf) = (&old.fill, &new.fill);
    let mut wrote = false;
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
    if of.enabled != nf.enabled {
        write_bool(meta, KEY_FILL_ENABLED, nf.enabled);
        wrote = true;
    }
    if of.kind != nf.kind {
        write_text(meta, KEY_FILL_KIND, kind_text(nf.kind));
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
/// from its default, and the stop list verbatim with the same stop ids (a
/// stop id is unique within one object's list, not across the document).
pub(crate) fn write_style(meta: &LoroMap, style: &Style) {
    write_stroke_width(meta, style.stroke.width);
    write_color(meta, KEY_STROKE, style.stroke.color);
    let mut baseline = Style::default();
    baseline.stroke.width = style.stroke.width;
    baseline.stroke.color = style.stroke.color;
    write_changes(meta, &baseline, style);
    write_stops(meta, &style.fill.stops);
}

// ---------------------------------------------------------------------------
// The stop list
// ---------------------------------------------------------------------------

/// The `fill_stops` movable list of this object, if it has one.
pub(crate) fn stops_list(meta: &LoroMap) -> Option<LoroMovableList> {
    match meta.get(KEY_FILL_STOPS) {
        Some(ValueOrContainer::Container(Container::MovableList(list))) => Some(list),
        _ => None,
    }
}

/// The `fill_stops` list, created if the object has none yet. Only
/// [`write_stops`] creates it (the fill-mode switch and Split); two peers that
/// create it concurrently keep one container under the key.
fn ensure_stops_list(meta: &LoroMap) -> LoroMovableList {
    if let Some(list) = stops_list(meta) {
        return list;
    }
    // invariant: inserting a brand-new container under a fresh key on an
    // attached map cannot fail.
    #[allow(clippy::unwrap_used)]
    meta.insert_container(KEY_FILL_STOPS, LoroMovableList::new())
        .unwrap()
}

pub(crate) fn stop_map_at(list: &LoroMovableList, index: usize) -> Option<LoroMap> {
    match list.get(index) {
        Some(ValueOrContainer::Container(Container::Map(map))) => Some(map),
        _ => None,
    }
}

/// The list index of the stop with `id`, if the list holds it.
pub(crate) fn stop_index(list: &LoroMovableList, id: StopId) -> Option<usize> {
    (0..list.len())
        .find(|&index| stop_map_at(list, index).is_some_and(|map| read_stop_id(&map) == Some(id)))
}

fn write_stop_fields(map: &LoroMap, stop: &GradientStop) {
    write_text(map, KEY_STOP_ID, &stop.id.to_hex());
    write_number(map, KEY_STOP_POSITION, stop.position.get());
    write_color(map, KEY_STOP_COLOR, stop.color);
    write_number(map, KEY_STOP_OPACITY, stop.opacity.get());
}

/// Appends `stops` in order to the object's list (creating it when `stops`
/// is not empty). Writes nothing for an empty slice.
pub(crate) fn write_stops(meta: &LoroMap, stops: &[GradientStop]) {
    if stops.is_empty() {
        return;
    }
    let list = ensure_stops_list(meta);
    for stop in stops {
        // invariant: push_container onto an attached movable list cannot
        // fail.
        #[allow(clippy::unwrap_used)]
        let map = list.push_container(LoroMap::new()).unwrap();
        write_stop_fields(&map, stop);
    }
}

/// Inserts `stop` at list `index`.
pub(crate) fn insert_stop_at(list: &LoroMovableList, index: usize, stop: &GradientStop) {
    // invariant: `index` is at most `len()`, which the caller takes from the
    // same list.
    #[allow(clippy::unwrap_used)]
    let map = list.insert_container(index, LoroMap::new()).unwrap();
    write_stop_fields(&map, stop);
}

/// Writes one stop's position.
pub(crate) fn write_stop_position(map: &LoroMap, position: StopPosition) {
    write_number(map, KEY_STOP_POSITION, position.get());
}

/// Writes one stop's colour.
pub(crate) fn write_stop_color(map: &LoroMap, color: Color) {
    write_color(map, KEY_STOP_COLOR, color);
}

/// Writes one stop's opacity.
pub(crate) fn write_stop_opacity(map: &LoroMap, opacity: Opacity) {
    write_number(map, KEY_STOP_OPACITY, opacity.get());
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
