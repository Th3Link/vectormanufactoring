//! The style schema's pure data types (`specs/0007-stroke-and-fill-styling/
//! adrs.md`, "the style schema"): one [`Style`] that every object node
//! carries, path or primitive, and the validated newtypes inside it. No Loro
//! type appears here; the CRDT wiring lives in [`crate::style_codec`].

use serde::{Deserialize, Serialize};

use crate::path_model::Color;
use crate::units::Length;

/// The stroke width every object is created with (`path-node-editing`'s and
/// `primitive-shapes`' placeholder, now the frozen default of an absent
/// `stroke_width` key).
pub(crate) const DEFAULT_STROKE_WIDTH_MM: f64 = 0.25;

/// Why building a validated style value failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StyleParamError {
    /// An opacity must be a finite number in `[0, 1]`.
    #[error("opacity must be between 0 and 1")]
    OpacityOutOfRange,
    /// A dash pattern needs finite, non-negative entries that do not sum to
    /// zero (or be empty, which is solid).
    #[error("a dash pattern needs non-negative lengths with a positive sum")]
    InvalidDashPattern,
    /// A marker count must be a whole number of at least 1.
    #[error("a marker count must be at least 1")]
    MarkerCountBelowOne,
}

/// An opacity, validated to `[0, 1]` and finite at construction (`1` is
/// opaque). Stored as a fraction; the panel shows it as an integer percent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Opacity(f64);

impl Opacity {
    /// Fully opaque, the default of every paint.
    pub const OPAQUE: Self = Self(1.0);

    /// Validates `value` against `[0, 1]`, finite.
    ///
    /// # Errors
    /// [`StyleParamError::OpacityOutOfRange`] otherwise.
    pub fn new(value: f64) -> Result<Self, StyleParamError> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(StyleParamError::OpacityOutOfRange)
        }
    }

    /// The validated fraction.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Opacity {
    type Error = StyleParamError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Opacity> for f64 {
    fn from(opacity: Opacity) -> Self {
        opacity.0
    }
}

/// A stroke's dash pattern: an ordered list of on, off, ... lengths in
/// **multiples of the stroke width**, not millimetres, so a width change
/// rescales the pattern with no second write (`adrs.md`, register
/// granularity 4). The empty list is solid. The list is unbounded; a later
/// numeric editor reads and writes the same field.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(try_from = "Vec<f64>", into = "Vec<f64>")]
pub struct DashPattern(Vec<f64>);

impl DashPattern {
    /// Solid: no dashes.
    #[must_use]
    pub const fn solid() -> Self {
        Self(Vec::new())
    }

    /// Validates `lengths`: empty (solid), or finite, non-negative entries
    /// whose sum is greater than zero. An odd count is kept as stored; the
    /// renderer repeats the list to make it even, as SVG does.
    ///
    /// # Errors
    /// [`StyleParamError::InvalidDashPattern`] otherwise.
    pub fn new(lengths: Vec<f64>) -> Result<Self, StyleParamError> {
        if lengths.is_empty() {
            return Ok(Self(lengths));
        }
        let entries_ok = lengths.iter().all(|n| n.is_finite() && *n >= 0.0);
        if entries_ok && lengths.iter().sum::<f64>() > 0.0 {
            Ok(Self(lengths))
        } else {
            Err(StyleParamError::InvalidDashPattern)
        }
    }

    /// The on, off, ... lengths as multiples of the stroke width.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.0
    }

    /// Whether this is the solid (empty) pattern.
    #[must_use]
    pub fn is_solid(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<Vec<f64>> for DashPattern {
    type Error = StyleParamError;

    fn try_from(lengths: Vec<f64>) -> Result<Self, Self::Error> {
        Self::new(lengths)
    }
}

impl From<DashPattern> for Vec<f64> {
    fn from(pattern: DashPattern) -> Self {
        pattern.0
    }
}

/// How two stroke segments meet at a vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineJoin {
    /// The stroke edges are extended to a point (limited by a fixed miter
    /// limit of 4 at render time).
    #[default]
    Miter,
    /// A circular arc around the vertex.
    Round,
    /// A flat line across the corner.
    Bevel,
}

/// How an open stroke end, or a dash end, is finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineCap {
    /// Ends exactly at the endpoint.
    #[default]
    Butt,
    /// A semicircle beyond the endpoint.
    Round,
    /// A square beyond the endpoint.
    Square,
}

/// An object's stroke: width, colour, opacity, dash, join and cap, plus the
/// on/off flag that keeps the other values stored while the stroke is off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// Whether the stroke is painted (`false` is "No stroke").
    pub enabled: bool,
    /// The stroke width; always greater than zero (zero is `enabled = false`).
    pub width: Length,
    /// The stroke colour.
    pub color: Color,
    /// The stroke opacity.
    pub opacity: Opacity,
    /// The dash pattern in multiples of the width.
    pub dash: DashPattern,
    /// The join style at sharp corners.
    pub join: LineJoin,
    /// The cap style at open ends.
    pub cap: LineCap,
    /// The markers on a path's start, along it and at its end
    /// (`specs/0018-stroke-markers`). A primitive never holds any.
    #[serde(default)]
    pub markers: Markers,
}

/// The shape of one marker slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkerShape {
    /// No marker.
    #[default]
    None,
    /// A filled arrow, 4 stroke widths long and 3 wide.
    Arrow,
    /// A filled dot, 3 stroke widths across.
    Dot,
}

/// Where the Middle marker goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkerPlace {
    /// A count of markers spread evenly along the path's length.
    #[default]
    Spaced,
    /// One marker on each node (not the ends of an open path).
    #[serde(rename = "nodes")]
    AtNodes,
}

/// How many Middle markers a Spaced placement draws: a whole number, at
/// least 1. The editor limits typing to 500 and dragging to 50; a file may
/// hold more (at most 500 are drawn).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct MarkerCount(u32);

impl MarkerCount {
    /// One marker, the default.
    pub const ONE: Self = Self(1);

    /// Validates `count` against the lower bound.
    ///
    /// # Errors
    /// [`StyleParamError::MarkerCountBelowOne`] for `0`.
    pub const fn new(count: u32) -> Result<Self, StyleParamError> {
        if count >= 1 {
            Ok(Self(count))
        } else {
            Err(StyleParamError::MarkerCountBelowOne)
        }
    }

    /// The count.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for MarkerCount {
    fn default() -> Self {
        Self::ONE
    }
}

impl TryFrom<u32> for MarkerCount {
    type Error = StyleParamError;

    fn try_from(count: u32) -> Result<Self, Self::Error> {
        Self::new(count)
    }
}

impl From<MarkerCount> for u32 {
    fn from(count: MarkerCount) -> Self {
        count.0
    }
}

/// The five marker settings of a stroke. Place and Count stay stored while
/// the Middle slot is None or Place is At nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Markers {
    /// The marker on the first node of an open path.
    pub start: MarkerShape,
    /// The marker along the path or on its nodes.
    pub mid: MarkerShape,
    /// The marker on the last node of an open path.
    pub end: MarkerShape,
    /// Where the Middle marker goes.
    pub mid_place: MarkerPlace,
    /// How many Middle markers a Spaced placement draws.
    pub mid_count: MarkerCount,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            enabled: true,
            width: Length::from_mm(DEFAULT_STROKE_WIDTH_MM),
            color: Color::BLACK,
            opacity: Opacity::OPAQUE,
            dash: DashPattern::solid(),
            join: LineJoin::Miter,
            cap: LineCap::Butt,
            markers: Markers::default(),
        }
    }
}

/// An object's fill: the on/off flag, the solid colour and its opacity. The
/// colour and opacity stay stored while the fill is off, so switching it back on
/// restores them. A fill is None or Solid; there is nothing else
/// (`specs/0017-style-panel-rework/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    /// Whether the fill is painted (`false` is "None").
    pub enabled: bool,
    /// The solid colour.
    pub color: Color,
    /// The solid fill's opacity.
    pub opacity: Opacity,
}

impl Fill {
    /// Whether this fill paints anything: it is on. A fill at opacity 0 still
    /// counts, because it is a non-None fill (`0007` acceptance criterion 23).
    /// The one rule the renderer and the interior hit-test share.
    #[must_use]
    pub const fn paints(&self) -> bool {
        self.enabled
    }
}

impl Default for Fill {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Color::BLACK,
            opacity: Opacity::OPAQUE,
        }
    }
}

/// An object's whole style. Absent keys in a stored object read as
/// [`Style::default`] field by field, which equals what slices 2 and 3
/// rendered (0.25 mm solid black stroke, no fill), so an older file opens
/// unchanged (acceptance criterion 3). The defaults are part of the format
/// and frozen.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Style {
    /// The stroke.
    pub stroke: Stroke,
    /// The fill.
    pub fill: Fill,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_accepts_exactly_the_unit_interval() {
        for ok in [0.0, 0.25, 1.0] {
            assert!(Opacity::new(ok).is_ok());
        }
        for bad in [-0.001, 1.001, f64::NAN, f64::INFINITY] {
            assert_eq!(Opacity::new(bad), Err(StyleParamError::OpacityOutOfRange));
        }
    }

    #[test]
    fn dash_pattern_validation_follows_the_adr() {
        assert!(DashPattern::new(vec![]).is_ok());
        assert!(DashPattern::new(vec![6.0, 4.0]).is_ok());
        assert!(
            DashPattern::new(vec![0.0, 3.0]).is_ok(),
            "a zero on is legal"
        );
        assert!(DashPattern::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).is_ok());
        // Odd lists are valid (format version 9).
        assert!(DashPattern::new(vec![6.0]).is_ok());
        assert!(DashPattern::new(vec![6.0, 4.0, 1.0]).is_ok());
        for bad in [
            vec![0.0],
            vec![-1.0, 4.0],
            vec![0.0, 0.0],
            vec![f64::NAN, 1.0],
            vec![1.0, f64::INFINITY],
        ] {
            assert_eq!(
                DashPattern::new(bad.clone()),
                Err(StyleParamError::InvalidDashPattern),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_default_style_is_the_frozen_placeholder() {
        let style = Style::default();
        assert!(style.stroke.enabled);
        assert!((style.stroke.width.as_mm() - 0.25).abs() < f64::EPSILON);
        assert_eq!(style.stroke.color, Color::BLACK);
        assert_eq!(style.stroke.opacity, Opacity::OPAQUE);
        assert!(style.stroke.dash.is_solid());
        assert_eq!(style.stroke.join, LineJoin::Miter);
        assert_eq!(style.stroke.cap, LineCap::Butt);
        assert!(!style.fill.enabled);
        assert_eq!(style.fill.color, Color::BLACK);
        assert_eq!(style.fill.opacity, Opacity::OPAQUE);
    }

    #[test]
    fn a_fill_paints_when_it_is_on() {
        let mut fill = Fill::default();
        assert!(!fill.paints(), "off");
        fill.enabled = true;
        assert!(fill.paints(), "solid");
        fill.opacity = Opacity::new(0.0).unwrap();
        assert!(fill.paints(), "opacity 0 still counts");
    }

    #[test]
    fn deserializing_a_style_value_validates_it() {
        assert!(serde_json::from_str::<Opacity>("1.5").is_err());
        assert!(serde_json::from_str::<DashPattern>("[0.0]").is_err());
        assert!(
            serde_json::from_str::<DashPattern>("[1.0]").is_ok(),
            "odd lists are valid"
        );
        let style = Style::default();
        let json = serde_json::to_string(&style).unwrap();
        assert_eq!(serde_json::from_str::<Style>(&json).unwrap(), style);
    }
}
