//! What the Style panel shows for the objects it edits: every property as one
//! shared value or "Mixed" (`specs/0007-stroke-and-fill-styling` criteria 1, 5,
//! 13, 24; `specs/0017-style-panel-rework` criteria 5 to 9). A pure function
//! of the snapshots, so the DOM holds no editing logic.

use curvyo_document_core::{
    Color, DashPattern, Length, LineCap, LineJoin, MarkerCount, MarkerPlace, MarkerShape,
    ObjectSnapshot, Opacity, PathSnapshot, Style, StyleEdit,
};

use crate::dash_text::dash_text;
use crate::object_selection::objects_with_ids;
use crate::select_bar::BarValue;
use crate::style_scope::StyleScope;
use crate::value_scale::ValueField;

/// Two stroke widths closer than this are one value, millimetres.
const WIDTH_EQUAL_EPSILON_MM: f64 = 1e-9;
/// Two dash lengths (multiples of the width) closer than this are one value.
const DASH_EQUAL_EPSILON: f64 = 1e-9;

/// The named dash patterns the panel offers as buttons (criterion 28). A
/// stored pattern that is none of them presses no button (criterion 31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashChoice {
    /// No dashes.
    Solid,
    /// `[6, 4]`.
    Dash,
    /// `[1, 3]`.
    Dot,
    /// `[6, 3, 1, 3]`.
    DashDot,
}

impl DashChoice {
    const PRESETS: [(Self, &'static [f64]); 4] = [
        (Self::Solid, &[]),
        (Self::Dash, &[6.0, 4.0]),
        (Self::Dot, &[1.0, 3.0]),
        (Self::DashDot, &[6.0, 3.0, 1.0, 3.0]),
    ];

    /// The preset a stored pattern equals, `None` for any other pattern.
    #[must_use]
    pub fn from_pattern(pattern: &DashPattern) -> Option<Self> {
        Self::PRESETS
            .iter()
            .find(|(_, lengths)| same_dash(pattern.as_slice(), lengths))
            .map(|(choice, _)| *choice)
    }

    /// The pattern a preset stores.
    #[must_use]
    pub fn pattern(self) -> DashPattern {
        let lengths = Self::PRESETS
            .iter()
            .find(|(choice, _)| *choice == self)
            .map_or(&[][..], |(_, lengths)| lengths);
        // The presets are valid patterns (even, positive sum, or empty).
        DashPattern::new(lengths.to_vec()).unwrap_or_default()
    }

    /// The preset named by the host (`"solid"`, `"dash"`, `"dot"`,
    /// `"dash-dot"`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "solid" => Some(Self::Solid),
            "dash" => Some(Self::Dash),
            "dot" => Some(Self::Dot),
            "dash-dot" => Some(Self::DashDot),
            _ => None,
        }
    }
}

fn same_dash(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x - y).abs() <= DASH_EQUAL_EPSILON)
}

/// A colour and its alpha, the value of the hex field.
pub type Rgba = (Color, Opacity);

/// What the Dash rows show (criteria 29, 31, 33).
#[derive(Debug, Clone, PartialEq)]
pub enum DashShown {
    /// Every edited stroke has the same list: the preset it equals (none for
    /// any other list) and the numbers as the text line shows them.
    Uniform {
        /// The pressed button, if the list equals a preset.
        preset: Option<DashChoice>,
        /// The numbers, one space apart; empty for solid.
        text: String,
    },
    /// The lists differ: no button pressed, the line reads "Mixed".
    Mixed,
}

/// The stroke rows of the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokePanel {
    /// The Paint switch: on, off, or "no state pressed".
    pub paint: BarValue<bool>,
    /// The rows below the switch are shown: they are not while every edited
    /// stroke is off (criterion 5). Decided from the committed document.
    pub rows_shown: bool,
    /// The colour, red green blue only (the picker's value).
    pub color: BarValue<Color>,
    /// The colour with its alpha (the hex field's value).
    pub rgba: BarValue<Rgba>,
    /// The opacity.
    pub opacity: BarValue<Opacity>,
    /// The stored width, shown also while the stroke is off.
    pub width: BarValue<Length>,
    /// The dash pattern.
    pub dash: DashShown,
    /// The join.
    pub join: BarValue<LineJoin>,
    /// The cap.
    pub cap: BarValue<LineCap>,
    /// The Markers group (`specs/0018-stroke-markers`): present while the
    /// stroke rows are shown and the scope holds at least one path.
    pub markers: Option<MarkersPanel>,
}

/// What the Markers group shows, over the paths of the scope only (a primitive
/// has no markers, criteria 21 and 22).
#[derive(Debug, Clone, PartialEq)]
pub struct MarkersPanel {
    /// The Start slot.
    pub start: BarValue<MarkerShape>,
    /// The Middle slot.
    pub mid: BarValue<MarkerShape>,
    /// The End slot.
    pub end: BarValue<MarkerShape>,
    /// Where the Middle marker goes; only meaningful while `place_shown`.
    pub place: BarValue<MarkerPlace>,
    /// The Place group is shown: some path has a Middle shape (criterion 2).
    pub place_shown: bool,
    /// How many Middle markers a Spaced placement draws.
    pub count: BarValue<MarkerCount>,
    /// The Count field is shown: Place is shown and some path with a Middle
    /// shape is Spaced.
    pub count_shown: bool,
    /// Start or End is set and every path in scope is closed: one muted line
    /// says closed paths have no start or end (criterion 32).
    pub closed_note: bool,
}

/// The fill rows of the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct FillPanel {
    /// The Paint switch: on, off, or "no state pressed".
    pub paint: BarValue<bool>,
    /// The rows below the switch are shown (criterion 6).
    pub rows_shown: bool,
    /// The colour, red green blue only.
    pub color: BarValue<Color>,
    /// The colour with its alpha.
    pub rgba: BarValue<Rgba>,
    /// The opacity.
    pub opacity: BarValue<Opacity>,
}

/// Everything the panel shows for a selection it can edit.
#[derive(Debug, Clone, PartialEq)]
pub struct StylePanelState {
    /// The subject line.
    pub subject: String,
    /// The stroke rows.
    pub stroke: StrokePanel,
    /// The fill rows.
    pub fill: FillPanel,
}

impl StylePanelState {
    /// The state with a width being dragged shown as the drag has it: a width
    /// previewed at 0 reads 0 although its preview style keeps the last width
    /// (the stroke is off), so the field follows the drag (criterion 8).
    #[must_use]
    pub fn with_pending(mut self, pending: Option<&StyleEdit>) -> Self {
        if let Some(StyleEdit::StrokeWidth(width)) = pending {
            self.stroke.width = BarValue::Uniform(*width);
        }
        self
    }

    /// The value of a value field in the scale's unit, `None` while the edited
    /// objects differ (or the field has nothing to show).
    #[must_use]
    pub fn value_of(&self, field: ValueField) -> Option<f64> {
        let uniform = |value: BarValue<f64>| match value {
            BarValue::Uniform(v) => Some(v),
            BarValue::Mixed => None,
        };
        match field {
            ValueField::StrokeWidth => uniform(map_bar(self.stroke.width, Length::as_mm)),
            ValueField::StrokeOpacity => uniform(map_bar(self.stroke.opacity, percent_of)),
            ValueField::FillOpacity => uniform(map_bar(self.fill.opacity, percent_of)),
            ValueField::MarkerCount => uniform(map_bar(self.stroke.markers.as_ref()?.count, |c| {
                f64::from(c.get())
            })),
        }
    }
}

fn percent_of(opacity: Opacity) -> f64 {
    opacity.get() * 100.0
}

fn map_bar<T: Copy>(value: BarValue<T>, convert: impl Fn(T) -> f64) -> BarValue<f64> {
    match value {
        BarValue::Uniform(v) => BarValue::Uniform(convert(v)),
        BarValue::Mixed => BarValue::Mixed,
    }
}

/// The one value of `values` if they all agree, else `Mixed`. The caller
/// always passes at least one value.
fn shared_by<T: Copy>(values: &[T], same: impl Fn(T, T) -> bool) -> BarValue<T> {
    match values.split_first() {
        Some((first, rest)) if rest.iter().all(|v| same(*first, *v)) => BarValue::Uniform(*first),
        _ => BarValue::Mixed,
    }
}

fn shared<T: Copy + PartialEq>(values: &[T]) -> BarValue<T> {
    shared_by(values, |a, b| a == b)
}

fn styles_of<'a>(objects: &'a [ObjectSnapshot], scope: &StyleScope) -> Vec<&'a Style> {
    objects_with_ids(objects, &scope.ids)
        .into_iter()
        .map(ObjectSnapshot::style)
        .collect()
}

/// What the panel shows for the objects of `scope`, or `None` when there is
/// nothing to edit (the panel body is then empty or the Document section).
///
/// Which rows are shown follows `committed`, the objects as the document holds
/// them; the values follow `shown`, the same objects with a drag preview
/// applied. So a width dragged to 0 does not remove its own row mid-drag
/// (criterion 8).
#[must_use]
pub fn style_panel_state(
    committed: &[ObjectSnapshot],
    shown: &[ObjectSnapshot],
    scope: &StyleScope,
) -> Option<StylePanelState> {
    let styles = styles_of(shown, scope);
    if styles.is_empty() {
        return None;
    }
    let held = styles_of(committed, scope);
    Some(StylePanelState {
        subject: scope.subject.clone(),
        stroke: StrokePanel {
            paint: shared(&column(&styles, |s| s.stroke.enabled)),
            rows_shown: held.iter().any(|s| s.stroke.enabled),
            color: shared(&column(&styles, |s| s.stroke.color)),
            rgba: shared(&column(&styles, |s| (s.stroke.color, s.stroke.opacity))),
            opacity: shared(&column(&styles, |s| s.stroke.opacity)),
            width: shared_by(&column(&styles, |s| s.stroke.width), |a: Length, b| {
                (a.as_mm() - b.as_mm()).abs() <= WIDTH_EQUAL_EPSILON_MM
            }),
            dash: dash_of(&styles),
            join: shared(&column(&styles, |s| s.stroke.join)),
            cap: shared(&column(&styles, |s| s.stroke.cap)),
            markers: markers_panel(shown, scope, held.iter().any(|s| s.stroke.enabled)),
        },
        fill: FillPanel {
            paint: shared(&column(&styles, |s| s.fill.paints())),
            rows_shown: held.iter().any(|s| s.fill.paints()),
            color: shared(&column(&styles, |s| s.fill.color)),
            rgba: shared(&column(&styles, |s| (s.fill.color, s.fill.opacity))),
            opacity: shared(&column(&styles, |s| s.fill.opacity)),
        },
    })
}

fn markers_panel(
    shown: &[ObjectSnapshot],
    scope: &StyleScope,
    rows_shown: bool,
) -> Option<MarkersPanel> {
    let paths: Vec<&PathSnapshot> = objects_with_ids(shown, &scope.ids)
        .into_iter()
        .filter_map(|object| match object {
            ObjectSnapshot::Path(path) => Some(path),
            ObjectSnapshot::Primitive(_) => None,
        })
        .collect();
    if !rows_shown || paths.is_empty() {
        return None;
    }
    let markers: Vec<_> = paths.iter().map(|p| p.style.stroke.markers).collect();
    let with_mid: Vec<_> = markers
        .iter()
        .filter(|m| m.mid != MarkerShape::None)
        .collect();
    let place_shown = !with_mid.is_empty();
    Some(MarkersPanel {
        start: shared(&column_of(&markers, |m| m.start)),
        mid: shared(&column_of(&markers, |m| m.mid)),
        end: shared(&column_of(&markers, |m| m.end)),
        place: shared(&column_of(&markers, |m| m.mid_place)),
        place_shown,
        count: shared(&column_of(&markers, |m| m.mid_count)),
        count_shown: with_mid.iter().any(|m| m.mid_place == MarkerPlace::Spaced),
        closed_note: paths.iter().all(|p| p.closed)
            && markers
                .iter()
                .any(|m| m.start != MarkerShape::None || m.end != MarkerShape::None),
    })
}

fn column_of<T, U>(items: &[T], pick: impl Fn(&T) -> U) -> Vec<U> {
    items.iter().map(pick).collect()
}

fn column<T>(styles: &[&Style], pick: impl Fn(&Style) -> T) -> Vec<T> {
    styles.iter().map(|style| pick(style)).collect()
}

fn dash_of(styles: &[&Style]) -> DashShown {
    let patterns: Vec<&DashPattern> = styles.iter().map(|s| &s.stroke.dash).collect();
    match shared_by(&patterns, |a, b| same_dash(a.as_slice(), b.as_slice())) {
        BarValue::Uniform(pattern) => DashShown::Uniform {
            preset: DashChoice::from_pattern(pattern),
            text: dash_text(pattern),
        },
        BarValue::Mixed => DashShown::Mixed,
    }
}
