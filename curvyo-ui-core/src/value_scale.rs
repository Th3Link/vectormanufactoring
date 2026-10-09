//! The scales of the panel's value fields: how a position `p` along the field
//! maps to a value and back, the rounding grids and the arrow-key steps
//! (`specs/0017-style-panel-rework` criteria 36 to 47, 61). Both curves are
//! slightly logarithmic: fine near zero, coarser towards the top. The host owns
//! only the gesture (pointer, threshold, modifiers); it sends `p` or a step
//! count and the grid, and Rust maps, rounds and previews.

use curvyo_document_core::{Length, MarkerCount, Opacity, Style, StyleEdit};

use crate::style_entry::opacity_from_percent;

/// How fine a value is rounded: Shift is coarse, Ctrl (Cmd on macOS) is fine
/// (criteria 38, 43, 47).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grid {
    /// The plain grid.
    Normal,
    /// Shift held: ten times coarser.
    Coarse,
    /// Ctrl held: ten times finer.
    Fine,
}

impl Grid {
    /// The grid named by the host (`"normal"`, `"coarse"`, `"fine"`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "normal" => Some(Self::Normal),
            "coarse" => Some(Self::Coarse),
            "fine" => Some(Self::Fine),
            _ => None,
        }
    }
}

/// The width scale: `v = 20 (100^p - 1) / 99`, drag range 0 to 20 mm.
const WIDTH_DRAG_MAX_MM: f64 = 20.0;
const WIDTH_BASE: f64 = 100.0;
/// The opacity scale: `v = 100 (4^p - 1) / 3`, 0 to 100 percent.
const OPACITY_MAX: f64 = 100.0;
const OPACITY_BASE: f64 = 4.0;
/// The largest value a typed width may hold, millimetres.
const WIDTH_TYPED_MAX_MM: f64 = 1000.0;
/// The marker count scale: `v = 1 + 49 p`, drag 1 to 50, typed 1 to 500.
const COUNT_MIN: f64 = 1.0;
const COUNT_DRAG_MAX: f64 = 50.0;
const COUNT_TYPED_MAX: f64 = 500.0;

/// A scale: the mapping of `p` in `[0, 1]` to a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueScale {
    /// The stroke width, millimetres.
    StrokeWidth,
    /// An opacity, whole percent.
    Opacity,
    /// The number of Middle markers: a whole number, linear.
    MarkerCount,
}

impl ValueScale {
    /// The value at position `p` (clamped to `[0, 1]`), not yet rounded.
    #[must_use]
    pub fn value_at(self, p: f64) -> f64 {
        let p = if p.is_finite() {
            p.clamp(0.0, 1.0)
        } else {
            0.0
        };
        match self {
            Self::StrokeWidth => {
                WIDTH_DRAG_MAX_MM * (WIDTH_BASE.powf(p) - 1.0) / (WIDTH_BASE - 1.0)
            }
            Self::Opacity => OPACITY_MAX * (OPACITY_BASE.powf(p) - 1.0) / (OPACITY_BASE - 1.0),
            Self::MarkerCount => COUNT_MIN + (COUNT_DRAG_MAX - COUNT_MIN) * p,
        }
    }

    /// The position of `value` on the scale, `0` to `1`; a value above the drag
    /// maximum is `1` (a full bar, criterion 45).
    #[must_use]
    pub fn position_of(self, value: f64) -> f64 {
        if !value.is_finite() || value <= 0.0 {
            return 0.0;
        }
        let p = match self {
            Self::MarkerCount => (value - COUNT_MIN) / (COUNT_DRAG_MAX - COUNT_MIN),
            Self::StrokeWidth => {
                (1.0 + value * (WIDTH_BASE - 1.0) / WIDTH_DRAG_MAX_MM).ln() / WIDTH_BASE.ln()
            }
            Self::Opacity => {
                (1.0 + value * (OPACITY_BASE - 1.0) / OPACITY_MAX).ln() / OPACITY_BASE.ln()
            }
        };
        p.clamp(0.0, 1.0)
    }

    /// The value at the end of the scale (`End`, criterion 43).
    #[must_use]
    pub const fn scale_end(self) -> f64 {
        match self {
            Self::StrokeWidth => WIDTH_DRAG_MAX_MM,
            Self::Opacity => OPACITY_MAX,
            Self::MarkerCount => COUNT_DRAG_MAX,
        }
    }

    /// The smallest value of the scale (`Home`).
    #[must_use]
    pub const fn scale_start(self) -> f64 {
        match self {
            Self::StrokeWidth | Self::Opacity => 0.0,
            Self::MarkerCount => COUNT_MIN,
        }
    }

    /// The largest value that may be typed (`aria-valuemax`).
    #[must_use]
    pub const fn typed_max(self) -> f64 {
        match self {
            Self::StrokeWidth => WIDTH_TYPED_MAX_MM,
            Self::Opacity => OPACITY_MAX,
            Self::MarkerCount => COUNT_TYPED_MAX,
        }
    }

    /// What a reset sets (criterion 61).
    #[must_use]
    pub const fn default_value(self) -> f64 {
        match self {
            Self::StrokeWidth => 0.25,
            Self::Opacity => OPACITY_MAX,
            Self::MarkerCount => COUNT_MIN,
        }
    }

    /// The grid spacing, in the scale's own unit.
    fn spacing(self, grid: Grid) -> f64 {
        match (self, grid) {
            (Self::StrokeWidth, Grid::Normal) => 0.01,
            (Self::StrokeWidth, Grid::Coarse) => 0.1,
            (Self::StrokeWidth, Grid::Fine) => 0.001,
            (Self::Opacity | Self::MarkerCount, Grid::Coarse) => 10.0,
            (Self::Opacity | Self::MarkerCount, _) => 1.0,
        }
    }

    /// `value` rounded to the grid; a value that rounds to 0 is 0 (criterion
    /// 47). Never negative.
    #[must_use]
    pub fn round(self, value: f64, grid: Grid) -> f64 {
        if self == Self::MarkerCount {
            // A whole number, never below 1.
            return if value.is_finite() {
                (value / self.spacing(grid))
                    .round()
                    .mul_add(self.spacing(grid), 0.0)
                    .max(COUNT_MIN)
            } else {
                COUNT_MIN
            };
        }
        if !value.is_finite() || value <= 0.0 {
            return 0.0;
        }
        let spacing = self.spacing(grid);
        let rounded = (value / spacing).round() * spacing;
        // Three decimals are the finest grid; this drops the float dust of
        // `n x 0.01`.
        ((rounded * 1000.0).round() / 1000.0).max(0.0)
    }

    /// `steps` arrow-key steps from `value`, on the grid and clamped to
    /// `[0, typed_max]` (criterion 43).
    #[must_use]
    pub fn step(self, value: f64, steps: i32, grid: Grid) -> f64 {
        let moved = value + f64::from(steps) * self.spacing(grid);
        self.round(moved, grid).min(self.typed_max())
    }

    /// The value as the field shows it: a width with up to three decimals and
    /// no trailing zeros (a positive width that three decimals would show as 0
    /// keeps enough digits to show it), an opacity as an integer.
    #[must_use]
    pub fn text(self, value: f64) -> String {
        match self {
            Self::Opacity | Self::MarkerCount => format!("{}", value.round()),
            Self::StrokeWidth => {
                let rounded = (value * 1000.0).round() / 1000.0;
                if rounded != 0.0 || value <= 0.0 || !value.is_finite() {
                    return format!("{}", rounded + 0.0);
                }
                // A positive width below 0.0005 mm: keep two significant digits.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let digits = (2.0 - value.log10().floor()).clamp(0.0, 20.0) as usize;
                let text = format!("{value:.digits$}");
                text.trim_end_matches('0').to_string()
            }
        }
    }
}

/// A property edited by a value field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueField {
    /// The stroke width.
    StrokeWidth,
    /// The stroke opacity.
    StrokeOpacity,
    /// The fill opacity.
    FillOpacity,
    /// The number of Middle markers.
    MarkerCount,
}

impl ValueField {
    /// The field named by the host (`"stroke-width"`, `"stroke-opacity"`,
    /// `"fill-opacity"`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "stroke-width" => Some(Self::StrokeWidth),
            "stroke-opacity" => Some(Self::StrokeOpacity),
            "fill-opacity" => Some(Self::FillOpacity),
            "marker-count" => Some(Self::MarkerCount),
            _ => None,
        }
    }

    /// The scale of the field.
    #[must_use]
    pub const fn scale(self) -> ValueScale {
        match self {
            Self::StrokeWidth => ValueScale::StrokeWidth,
            Self::StrokeOpacity | Self::FillOpacity => ValueScale::Opacity,
            Self::MarkerCount => ValueScale::MarkerCount,
        }
    }

    /// The edit that sets `value` (millimetres for a width, percent for an
    /// opacity).
    #[must_use]
    pub fn edit(self, value: f64) -> StyleEdit {
        match self {
            Self::StrokeWidth => StyleEdit::StrokeWidth(Length::from_mm(value)),
            Self::StrokeOpacity => StyleEdit::StrokeOpacity(opacity_from_percent(value)),
            Self::FillOpacity => StyleEdit::FillOpacity(opacity_from_percent(value)),
            Self::MarkerCount => StyleEdit::MarkerCount(count_from(value)),
        }
    }

    /// The edit that resets the field to its default (criterion 61).
    #[must_use]
    pub fn reset_edit(self) -> StyleEdit {
        self.edit(self.scale().default_value())
    }

    /// The value of the field in `style`, in the scale's unit.
    #[must_use]
    pub fn value_in(self, style: &Style) -> f64 {
        match self {
            Self::StrokeWidth => style.stroke.width.as_mm(),
            Self::StrokeOpacity => percent(style.stroke.opacity),
            Self::FillOpacity => percent(style.fill.opacity),
            Self::MarkerCount => f64::from(style.stroke.markers.mid_count.get()),
        }
    }
}

/// A marker count from a rounded value: a whole number of at least 1.
fn count_from(value: f64) -> MarkerCount {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = value.round().clamp(COUNT_MIN, COUNT_TYPED_MAX) as u32;
    MarkerCount::new(whole).unwrap_or(MarkerCount::ONE)
}

fn percent(opacity: Opacity) -> f64 {
    opacity.get() * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, tolerance: f64) {
        assert!((a - b).abs() <= tolerance, "{a} vs {b}");
    }

    #[test]
    fn the_width_check_values_of_criterion_46() {
        let width = ValueScale::StrokeWidth;
        assert_eq!(width.value_at(0.0), 0.0);
        near(width.value_at(0.1749), 0.25, 0.001);
        near(width.value_at(0.5), 1.82, 0.005);
        near(width.value_at(1.0), 20.0, 1e-9);
    }

    #[test]
    fn the_opacity_check_values_of_criterion_46() {
        let opacity = ValueScale::Opacity;
        assert_eq!(opacity.value_at(0.0), 0.0);
        assert_eq!(opacity.round(opacity.value_at(0.5), Grid::Normal), 33.0);
        assert_eq!(opacity.round(opacity.value_at(0.9), Grid::Normal), 83.0);
        near(opacity.value_at(1.0), 100.0, 1e-9);
    }

    #[test]
    fn the_inverse_draws_the_bar_where_the_value_came_from() {
        for scale in [ValueScale::StrokeWidth, ValueScale::Opacity] {
            for step in 0..=100 {
                let p = f64::from(step) / 100.0;
                near(scale.position_of(scale.value_at(p)), p, 1e-9);
            }
        }
        let width = ValueScale::StrokeWidth;
        // The bar positions of the UX notes at W = 244: 0.25 mm is 43 px.
        near(width.position_of(0.25) * 244.0, 43.0, 0.6);
        near(width.position_of(1.0) * 244.0, 95.0, 0.6);
        near(width.position_of(5.0) * 244.0, 172.0, 0.6);
    }

    #[test]
    fn a_value_above_the_drag_maximum_draws_a_full_bar() {
        assert_eq!(ValueScale::StrokeWidth.position_of(500.0), 1.0);
        assert_eq!(ValueScale::StrokeWidth.position_of(0.0), 0.0);
        assert_eq!(ValueScale::Opacity.position_of(f64::NAN), 0.0);
    }

    #[test]
    fn the_drag_examples_of_criterion_36() {
        // Width, W = 244: from 0.25 mm, 24.4 px right gives 0.51 mm.
        let width = ValueScale::StrokeWidth;
        let p = width.position_of(0.25) + 24.4 / 244.0;
        near(width.round(width.value_at(p), Grid::Normal), 0.51, 0.011);
        // Opacity: from 100, 24.4 px left gives 83.
        let opacity = ValueScale::Opacity;
        let p = opacity.position_of(100.0) - 24.4 / 244.0;
        assert_eq!(opacity.round(opacity.value_at(p), Grid::Normal), 83.0);
    }

    #[test]
    fn the_grids_of_criterion_47() {
        let width = ValueScale::StrokeWidth;
        assert_eq!(width.round(0.2549, Grid::Normal), 0.25);
        assert_eq!(width.round(0.2551, Grid::Normal), 0.26);
        assert_eq!(width.round(0.2549, Grid::Coarse), 0.3);
        assert_eq!(width.round(0.12349, Grid::Fine), 0.123);
        assert_eq!(width.round(0.004, Grid::Normal), 0.0, "rounds to 0 is 0");
        assert_eq!(width.round(0.04, Grid::Coarse), 0.0);
        let opacity = ValueScale::Opacity;
        assert_eq!(opacity.round(49.6, Grid::Normal), 50.0);
        assert_eq!(opacity.round(49.6, Grid::Coarse), 50.0);
        assert_eq!(opacity.round(0.4, Grid::Fine), 0.0);
    }

    #[test]
    fn the_left_end_of_the_scale_is_exactly_zero() {
        assert_eq!(
            ValueScale::StrokeWidth.round(ValueScale::StrokeWidth.value_at(0.0), Grid::Normal),
            0.0
        );
        assert_eq!(
            ValueScale::Opacity.round(ValueScale::Opacity.value_at(0.0), Grid::Normal),
            0.0
        );
    }

    #[test]
    fn arrow_steps_follow_the_grid_and_clamp() {
        let width = ValueScale::StrokeWidth;
        assert_eq!(width.step(0.25, 1, Grid::Normal), 0.26);
        assert_eq!(width.step(0.25, -1, Grid::Normal), 0.24);
        assert_eq!(
            width.step(0.25, 1, Grid::Coarse),
            0.3,
            "0.35 rounds on the 0.1 grid"
        );
        assert_eq!(width.step(0.25, 1, Grid::Fine), 0.251);
        assert_eq!(width.step(0.0, -1, Grid::Normal), 0.0);
        assert_eq!(width.step(999.999, 5, Grid::Coarse), 1000.0);
        let opacity = ValueScale::Opacity;
        assert_eq!(opacity.step(50.0, 1, Grid::Normal), 51.0);
        assert_eq!(opacity.step(50.0, 1, Grid::Coarse), 60.0);
        assert_eq!(
            opacity.step(50.0, 1, Grid::Fine),
            51.0,
            "never below the grid"
        );
        assert_eq!(opacity.step(100.0, 1, Grid::Normal), 100.0);
        assert_eq!(opacity.step(0.0, -1, Grid::Normal), 0.0);
    }

    #[test]
    fn the_marker_count_scale_is_linear_and_whole() {
        let count = ValueScale::MarkerCount;
        assert_eq!(count.round(count.value_at(0.0), Grid::Normal), 1.0);
        assert_eq!(count.round(count.value_at(1.0), Grid::Normal), 50.0);
        // 5 px per step at 244 px.
        let step = count.position_of(2.0) * 244.0 - count.position_of(1.0) * 244.0;
        near(step, 244.0 / 49.0, 1e-9);
        assert_eq!(count.round(0.2, Grid::Normal), 1.0, "never below 1");
        assert_eq!(count.round(f64::NAN, Grid::Normal), 1.0);
        assert_eq!(count.step(1.0, -1, Grid::Normal), 1.0);
        assert_eq!(count.step(499.0, 5, Grid::Normal), 500.0);
        assert_eq!(
            count.step(5.0, 1, Grid::Coarse),
            20.0,
            "15 rounds on the 10 grid"
        );
        assert_eq!(
            count.position_of(300.0),
            1.0,
            "a full bar above the drag range"
        );
        assert_eq!(count.text(3.0), "3");
        assert_eq!(count.default_value(), 1.0);
        assert_eq!(count.typed_max(), 500.0);
        assert_eq!(count.scale_start(), 1.0);
        assert_eq!(
            ValueField::MarkerCount.edit(7.4),
            StyleEdit::MarkerCount(MarkerCount::new(7).unwrap())
        );
    }

    #[test]
    fn home_and_end_values() {
        assert_eq!(ValueScale::StrokeWidth.scale_end(), 20.0);
        assert_eq!(ValueScale::Opacity.scale_end(), 100.0);
        assert_eq!(ValueScale::StrokeWidth.typed_max(), 1000.0);
        assert_eq!(ValueScale::Opacity.typed_max(), 100.0);
    }

    #[test]
    fn width_text_has_up_to_three_decimals_and_no_trailing_zeros() {
        let width = ValueScale::StrokeWidth;
        assert_eq!(width.text(0.25), "0.25");
        assert_eq!(width.text(1.82), "1.82");
        assert_eq!(width.text(0.125), "0.125");
        assert_eq!(width.text(2.0), "2");
        assert_eq!(width.text(0.0), "0");
        assert_eq!(width.text(0.12345), "0.123");
        assert_eq!(
            width.text(0.0004),
            "0.0004",
            "a stroke that is on never reads as 0"
        );
        assert_eq!(ValueScale::Opacity.text(49.6), "50");
        assert_eq!(ValueScale::Opacity.text(100.0), "100");
    }

    #[test]
    fn a_field_builds_its_edit_and_reads_its_value() {
        let mut style = Style::default();
        for (field, shown) in [
            (ValueField::StrokeWidth, 0.25),
            (ValueField::StrokeOpacity, 100.0),
            (ValueField::FillOpacity, 100.0),
        ] {
            assert_eq!(field.value_in(&style), shown);
        }
        ValueField::StrokeWidth
            .edit(3.0)
            .apply_to(&mut style)
            .unwrap();
        ValueField::FillOpacity
            .edit(40.0)
            .apply_to(&mut style)
            .unwrap();
        assert_eq!(ValueField::StrokeWidth.value_in(&style), 3.0);
        assert_eq!(ValueField::FillOpacity.value_in(&style), 40.0);
        ValueField::StrokeWidth
            .reset_edit()
            .apply_to(&mut style)
            .unwrap();
        assert_eq!(style.stroke.width, Length::from_mm(0.25));
        assert_eq!(
            ValueField::from_name("stroke-width"),
            Some(ValueField::StrokeWidth)
        );
        assert_eq!(ValueField::from_name("fill-width"), None);
        assert_eq!(Grid::from_name("fine"), Some(Grid::Fine));
        assert_eq!(Grid::from_name("x"), None);
    }
}
