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
    /// A gradient stop position must be a finite number in `[0, 1]`.
    #[error("stop position must be between 0 and 1")]
    StopPositionOutOfRange,
    /// A dash pattern needs an even number of finite, non-negative entries
    /// that do not sum to zero (or be empty, which is solid).
    #[error("a dash pattern needs an even number of non-negative lengths with a positive sum")]
    InvalidDashPattern,
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

/// A gradient stop's position along the gradient, validated to `[0, 1]` and
/// finite at construction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct StopPosition(f64);

impl StopPosition {
    /// The start of the gradient.
    pub const START: Self = Self(0.0);
    /// The end of the gradient.
    pub const END: Self = Self(1.0);

    /// Validates `value` against `[0, 1]`, finite.
    ///
    /// # Errors
    /// [`StyleParamError::StopPositionOutOfRange`] otherwise.
    pub fn new(value: f64) -> Result<Self, StyleParamError> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(StyleParamError::StopPositionOutOfRange)
        }
    }

    /// The validated position.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for StopPosition {
    type Error = StyleParamError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<StopPosition> for f64 {
    fn from(position: StopPosition) -> Self {
        position.0
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

    /// Validates `lengths`: empty (solid), or an even number of finite,
    /// non-negative entries whose sum is greater than zero.
    ///
    /// # Errors
    /// [`StyleParamError::InvalidDashPattern`] otherwise.
    pub fn new(lengths: Vec<f64>) -> Result<Self, StyleParamError> {
        if lengths.is_empty() {
            return Ok(Self(lengths));
        }
        let entries_ok = lengths.iter().all(|n| n.is_finite() && *n >= 0.0);
        if lengths.len().is_multiple_of(2) && entries_ok && lengths.iter().sum::<f64>() > 0.0 {
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

/// Which kind of paint a fill is when it is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FillKind {
    /// One colour with its own opacity.
    #[default]
    Solid,
    /// Stops interpolated along the default axis of the selection box.
    Linear,
    /// Stops interpolated outward from the centre of the selection box.
    Radial,
}

/// A gradient stop's identity, unique within **one object's** stop list.
///
/// Commands and the panel's selected-stop state address a stop as
/// `(NodeId, StopId)`; a copy of an object keeps its stops' ids
/// (`adrs.md`, 2026-10-07 readiness check, section 3). Built from a peer id
/// and a per-session counter like [`crate::AnchorId`], and minted by the
/// caller, never here (`CLAUDE.md` §6).
///
/// `(De)Serialize` write the same 32-digit hex string as the Loro storage,
/// so a non-Rust reader of `document.json` never meets a wide JSON number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StopId(u128);

impl StopId {
    /// Builds a [`StopId`] from a peer id and a counter that peer never
    /// reuses within one open session.
    #[must_use]
    pub const fn new(peer: u64, counter: u64) -> Self {
        Self(((peer as u128) << 64) | counter as u128)
    }

    /// The raw value.
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }

    /// The lowercase, zero-padded 32-digit hex encoding used in storage and
    /// in `document.json`.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("{:032x}", self.0)
    }

    /// Parses [`StopId::to_hex`]'s encoding.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        u128::from_str_radix(hex, 16).ok().map(Self)
    }
}

impl Serialize for StopId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for StopId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid StopId hex string"))
    }
}

/// One stop of a gradient fill. Its opacity is its own, independent of the
/// fill: a gradient has no top-level alpha.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    /// This stop's identity within its object's stop list.
    pub id: StopId,
    /// Where along the gradient the stop sits.
    pub position: StopPosition,
    /// The stop's colour.
    pub color: Color,
    /// The stop's own opacity.
    pub opacity: Opacity,
}

impl GradientStop {
    /// The two stops a new gradient starts with (acceptance criterion 17):
    /// position 0 takes `fill_color` at full opacity; position 1 is white at
    /// full opacity, or black if `fill_color` is white, so the ramp is always
    /// visible against the canvas.
    #[must_use]
    pub const fn default_pair(fill_color: Color, first: StopId, second: StopId) -> [Self; 2] {
        let white = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let end_color =
            if fill_color.r == white.r && fill_color.g == white.g && fill_color.b == white.b {
                Color::BLACK
            } else {
                white
            };
        [
            Self {
                id: first,
                position: StopPosition::START,
                color: fill_color,
                opacity: Opacity::OPAQUE,
            },
            Self {
                id: second,
                position: StopPosition::END,
                color: end_color,
                opacity: Opacity::OPAQUE,
            },
        ]
    }
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
        }
    }
}

/// An object's fill: the on/off flag, the kind, the solid colour and
/// opacity, and the gradient stops. The values of the kinds not in use stay
/// stored, so switching the kind back loses nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    /// Whether the fill is painted (`false` is "None").
    pub enabled: bool,
    /// Which paint is used while the fill is on.
    pub kind: FillKind,
    /// The solid colour.
    pub color: Color,
    /// The solid fill's opacity.
    pub opacity: Opacity,
    /// The gradient stops in list order (render order is a stable sort by
    /// position). Any count is valid in a stored document; 2 to 16 is an
    /// edit-time rule.
    pub stops: Vec<GradientStop>,
}

impl Default for Fill {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: FillKind::Solid,
            color: Color::BLACK,
            opacity: Opacity::OPAQUE,
            stops: Vec::new(),
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
    fn opacity_and_position_accept_exactly_the_unit_interval() {
        for ok in [0.0, 0.25, 1.0] {
            assert!(Opacity::new(ok).is_ok());
            assert!(StopPosition::new(ok).is_ok());
        }
        for bad in [-0.001, 1.001, f64::NAN, f64::INFINITY] {
            assert_eq!(Opacity::new(bad), Err(StyleParamError::OpacityOutOfRange));
            assert_eq!(
                StopPosition::new(bad),
                Err(StyleParamError::StopPositionOutOfRange)
            );
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
        for bad in [
            vec![6.0],
            vec![6.0, 4.0, 1.0],
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
        assert_eq!(style.fill.kind, FillKind::Solid);
        assert_eq!(style.fill.color, Color::BLACK);
        assert_eq!(style.fill.opacity, Opacity::OPAQUE);
        assert_eq!(style.fill.stops.len(), 0);
    }

    #[test]
    fn stop_id_round_trips_through_hex_and_json() {
        let id = StopId::new(u64::MAX, 7);
        assert_eq!(StopId::from_hex(&id.to_hex()), Some(id));
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{}\"", id.to_hex()));
        assert_eq!(serde_json::from_str::<StopId>(&json).unwrap(), id);
    }

    /// Acceptance criterion 17: red becomes red to white, white becomes
    /// white to black, both opaque, at positions 0 and 1.
    #[test]
    fn a_new_gradient_starts_with_the_fill_colour_and_white_or_black() {
        let red = Color { r: 255, g: 0, b: 0 };
        let white = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let (a, b) = (StopId::new(1, 1), StopId::new(1, 2));
        let [first, second] = GradientStop::default_pair(red, a, b);
        assert_eq!(
            (first.id, first.position, first.color),
            (a, StopPosition::START, red)
        );
        assert_eq!(
            (second.id, second.position, second.color),
            (b, StopPosition::END, white)
        );
        assert_eq!(first.opacity, Opacity::OPAQUE);
        assert_eq!(second.opacity, Opacity::OPAQUE);
        let [_, end] = GradientStop::default_pair(white, a, b);
        assert_eq!(end.color, Color::BLACK);
        let [start, _] = GradientStop::default_pair(Color::BLACK, a, b);
        assert_eq!(start.color, Color::BLACK, "black stays black at stop 0");
    }

    #[test]
    fn deserializing_a_style_value_validates_it() {
        assert!(serde_json::from_str::<Opacity>("1.5").is_err());
        assert!(serde_json::from_str::<StopPosition>("-0.1").is_err());
        assert!(serde_json::from_str::<DashPattern>("[1.0]").is_err());
        let style = Style::default();
        let json = serde_json::to_string(&style).unwrap();
        assert_eq!(serde_json::from_str::<Style>(&json).unwrap(), style);
    }
}
