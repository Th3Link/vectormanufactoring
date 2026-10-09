//! Strict open-file validation of the style keys
//! (`specs/0007-stroke-and-fill-styling/adrs.md`, "`format_version`").

use loro::{LoroMap, LoroValue};

use crate::legacy_fill::{KEY_FILL_KIND, is_valid_kind};
use crate::path_codec::as_f64;
use crate::style_codec::{
    CAP_BUTT, CAP_ROUND, CAP_SQUARE, JOIN_BEVEL, JOIN_MITER, JOIN_ROUND, KEY_FILL,
    KEY_FILL_ENABLED, KEY_FILL_OPACITY, KEY_STROKE, KEY_STROKE_CAP, KEY_STROKE_DASH,
    KEY_STROKE_ENABLED, KEY_STROKE_JOIN, KEY_STROKE_OPACITY, KEY_STROKE_WIDTH, read_value,
};
use crate::style_model::DashPattern;

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
/// and range. `fill_kind` may hold any kind a version-7 build wrote, and
/// `fill_stops` is accepted in any form and never read (`legacy_fill`).
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
        && key_ok(meta, KEY_FILL_KIND, is_valid_kind)
        && key_ok(meta, KEY_FILL, is_color)
        && key_ok(meta, KEY_FILL_OPACITY, is_unit_number)
}
