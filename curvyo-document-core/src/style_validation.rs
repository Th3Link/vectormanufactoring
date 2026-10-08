//! Strict open-file validation of the style keys and the stop list
//! (`specs/0007-stroke-and-fill-styling/adrs.md`, "`format_version`").

use loro::{Container, LoroMap, LoroValue, ValueOrContainer};

use crate::path_codec::as_f64;
use crate::style_codec::{
    CAP_BUTT, CAP_ROUND, CAP_SQUARE, JOIN_BEVEL, JOIN_MITER, JOIN_ROUND, KEY_FILL,
    KEY_FILL_ENABLED, KEY_FILL_KIND, KEY_FILL_OPACITY, KEY_FILL_STOPS, KEY_STOP_COLOR, KEY_STOP_ID,
    KEY_STOP_OPACITY, KEY_STOP_POSITION, KEY_STROKE, KEY_STROKE_CAP, KEY_STROKE_DASH,
    KEY_STROKE_ENABLED, KEY_STROKE_JOIN, KEY_STROKE_OPACITY, KEY_STROKE_WIDTH, KIND_LINEAR,
    KIND_RADIAL, KIND_SOLID, read_value, stop_map_at,
};
use crate::style_model::{DashPattern, StopId};

/// A key that is absent is valid; a present key must satisfy `ok`.
fn key_ok(meta: &LoroMap, key: &str, ok: impl Fn(&LoroValue) -> bool) -> bool {
    read_value(meta, key).is_none_or(|v| ok(&v))
}

fn number_in(value: &LoroValue, accept: impl Fn(f64) -> bool) -> bool {
    as_f64(value).is_some_and(|n| n.is_finite() && accept(n))
}

fn is_unit_number(value: &LoroValue) -> bool {
    number_in(value, |n| (0.0..=1.0).contains(&n))
}

fn is_color(value: &LoroValue) -> bool {
    match value {
        LoroValue::List(list) if list.len() == 3 => list
            .iter()
            .all(|c| matches!(c, LoroValue::I64(n) if (0..=255).contains(n))),
        _ => false,
    }
}

fn is_one_of(value: &LoroValue, names: &[&str]) -> bool {
    matches!(value, LoroValue::String(s) if names.contains(&s.as_str()))
}

fn is_dash(value: &LoroValue) -> bool {
    match value {
        LoroValue::List(list) => list
            .iter()
            .map(as_f64)
            .collect::<Option<Vec<f64>>>()
            .is_some_and(|lengths| DashPattern::new(lengths).is_ok()),
        _ => false,
    }
}

fn is_bool(value: &LoroValue) -> bool {
    matches!(value, LoroValue::Bool(_))
}

/// Whether every style key of this object that is present has the right type
/// and range, and every stop in `fill_stops` is complete. The stop count is
/// never checked: a merged document may legitimately hold 0, 1 or more than
/// 16 stops.
pub(crate) fn style_is_valid(meta: &LoroMap) -> bool {
    key_ok(meta, KEY_STROKE_ENABLED, is_bool)
        && key_ok(meta, KEY_STROKE_WIDTH, |v| number_in(v, |n| n > 0.0))
        && key_ok(meta, KEY_STROKE, is_color)
        && key_ok(meta, KEY_STROKE_OPACITY, is_unit_number)
        && key_ok(meta, KEY_STROKE_DASH, is_dash)
        && key_ok(meta, KEY_STROKE_JOIN, |v| {
            is_one_of(v, &[JOIN_MITER, JOIN_ROUND, JOIN_BEVEL])
        })
        && key_ok(meta, KEY_STROKE_CAP, |v| {
            is_one_of(v, &[CAP_BUTT, CAP_ROUND, CAP_SQUARE])
        })
        && key_ok(meta, KEY_FILL_ENABLED, is_bool)
        && key_ok(meta, KEY_FILL_KIND, |v| {
            is_one_of(v, &[KIND_SOLID, KIND_LINEAR, KIND_RADIAL])
        })
        && key_ok(meta, KEY_FILL, is_color)
        && key_ok(meta, KEY_FILL_OPACITY, is_unit_number)
        && stops_are_valid(meta)
}

fn stops_are_valid(meta: &LoroMap) -> bool {
    match meta.get(KEY_FILL_STOPS) {
        None => true,
        Some(ValueOrContainer::Container(Container::MovableList(list))) => (0..list.len())
            .all(|index| stop_map_at(&list, index).is_some_and(|map| stop_is_valid(&map))),
        Some(_) => false,
    }
}

fn stop_is_valid(map: &LoroMap) -> bool {
    let present =
        |key: &str, ok: &dyn Fn(&LoroValue) -> bool| read_value(map, key).is_some_and(|v| ok(&v));
    present(
        KEY_STOP_ID,
        &|v| matches!(v, LoroValue::String(s) if StopId::from_hex(s.as_str()).is_some()),
    ) && present(KEY_STOP_POSITION, &is_unit_number)
        && present(KEY_STOP_COLOR, &is_color)
        && present(KEY_STOP_OPACITY, &is_unit_number)
}
