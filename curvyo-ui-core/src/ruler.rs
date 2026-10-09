//! The tick layout of one ruler: where the major and minor ticks and the labels
//! of a ruler strip go for a given view, axis, strip length and display unit
//! (`specs/0015-document-size-and-rulers/` criteria 3 to 7, `adrs.md`
//! decision 9).
//!
//! Pure arithmetic. Labels are built from integers (`index * mantissa`, with
//! the decimal point placed by the exponent), never by printing an `f64`, so
//! the tick at 0.3 reads "0.3" and no label has trailing zeros.

use curvyo_document_core::{DisplayUnit, Length, Point, ViewTransform};

/// The smallest on-screen distance between two major ticks, px.
pub(crate) const MIN_MAJOR_PX: f64 = 40.0;

/// How far right of (or below) its tick a label starts, px.
pub(crate) const LABEL_OFFSET_PX: f64 = 4.0;

/// The room kept between the widest label and the next major tick when the
/// step is chosen, px.
const STEP_LABEL_GAP_PX: f64 = 4.0;

/// The room kept between two drawn labels, px: when the major spacing is
/// smaller than the widest label plus this, labels go on every second, then
/// every fifth major tick.
const LABEL_GAP_PX: f64 = 8.0;

/// How far outside the strip the 0 tick may lie and still count as on it, px.
const ORIGIN_TOLERANCE_PX: f64 = 1.0;

/// The most major ticks one layout holds. A strip holds a few dozen (40 px
/// apart); more means a view outside any real zoom range, which gets an empty
/// layout instead of an allocation that never ends.
const MAX_MAJORS: i64 = 4096;

/// Minor intervals per major interval.
const MINORS_PER_MAJOR: u32 = 5;

/// The 1-2-5 mantissas of a step, ascending.
const MANTISSAS: [u32; 3] = [1, 2, 5];

/// The smallest and largest power of ten a step may have, in the display
/// unit. Wide enough for 2 % to 8000 % in mm, cm and in.
const MIN_EXPONENT: i32 = -6;
const MAX_EXPONENT: i32 = 12;

/// The U+2212 minus sign the labels use.
const MINUS: char = '\u{2212}';

/// Which ruler: the top one measures x, the left one measures y.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerAxis {
    /// The ruler along the top edge: x grows to the right.
    Horizontal,
    /// The ruler along the left edge: y grows downward.
    Vertical,
}

/// One major tick: its index (value = index times the step) and its position
/// along the strip, px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerMajor {
    /// The tick's index; the tick at 0 has index 0.
    pub index: i64,
    /// The tick's position along the strip, px from the strip's start.
    pub px: f64,
}

/// One drawn label.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerLabel {
    /// The position along the strip of the major tick the label belongs to.
    /// The label starts 4 px after it.
    pub tick_px: f64,
    /// The text: the value in the display unit, no unit suffix, U+2212 for
    /// a minus sign.
    pub text: String,
}

/// The layout of one ruler strip.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerLayout {
    /// The step's mantissa (1, 2 or 5) ...
    pub mantissa: u32,
    /// ... and power of ten, in the display unit: the step is
    /// `mantissa * 10^exponent`.
    pub exponent: i32,
    /// The distance between two major ticks, px.
    pub major_px: f64,
    /// The major ticks from just before the strip's start to just after its
    /// end (up to one step outside, so the minors next to the edge exist).
    pub majors: Vec<RulerMajor>,
    /// The minor ticks (four between two majors), px.
    pub minors: Vec<f64>,
    /// Labels are on every `label_every`-th major tick: 1, 2 or 5.
    pub label_every: u32,
    /// The labels that fit entirely inside the strip.
    pub labels: Vec<RulerLabel>,
    /// The position of the value 0 if it lies on the strip.
    pub origin_px: Option<f64>,
}

impl RulerLayout {
    /// The step in the display unit.
    #[must_use]
    pub fn step(&self) -> f64 {
        f64::from(self.mantissa) * 10f64.powi(self.exponent)
    }

    fn empty() -> Self {
        Self {
            mantissa: 1,
            exponent: 0,
            major_px: 0.0,
            majors: Vec::new(),
            minors: Vec::new(),
            label_every: 1,
            labels: Vec::new(),
            origin_px: None,
        }
    }
}

/// Lays out one ruler strip of `length_px` px along `axis` for `view`, with
/// labels in `unit`. `digit_px` is the advance of one digit of the label font;
/// every character of a label is counted as one digit wide, which is never
/// narrower than the real text.
///
/// The step is the smallest 1, 2 or 5 times a power of ten (in `unit`) whose
/// major spacing is at least [`MIN_MAJOR_PX`] and at least the widest visible
/// label plus 4 px (criterion 5). Labels that would not lie entirely inside
/// the strip are left out (criterion 7).
#[must_use]
pub fn ruler_layout(
    view: ViewTransform,
    axis: RulerAxis,
    length_px: f64,
    unit: DisplayUnit,
    digit_px: f64,
) -> RulerLayout {
    let px_per_unit = view.scale() * unit.mm_per_unit();
    if !(length_px > 0.0 && length_px.is_finite() && px_per_unit.is_finite() && px_per_unit > 0.0) {
        return RulerLayout::empty();
    }
    let along = |point: Point| match axis {
        RulerAxis::Horizontal => point.x,
        RulerAxis::Vertical => point.y,
    };
    let to_px = |value: f64| {
        let mm = Length::from_unit(value, unit).as_mm();
        let (x, y) = view.document_to_screen(Point::new(mm, mm));
        match axis {
            RulerAxis::Horizontal => x,
            RulerAxis::Vertical => y,
        }
    };
    let from_px = |px: f64| Length::from_mm(along(view.screen_to_document(px, px))).in_unit(unit);
    let (start, end) = (from_px(0.0), from_px(length_px));
    if !(start.is_finite() && end.is_finite()) {
        return RulerLayout::empty();
    }

    let Some(Step {
        mantissa,
        exponent,
        first,
        last,
        widest,
    }) = choose_step(px_per_unit, start, end, digit_px)
    else {
        return RulerLayout::empty();
    };
    let step = f64::from(mantissa) * 10f64.powi(exponent);
    let major_px = step * px_per_unit;

    let majors: Vec<RulerMajor> = (first..=last)
        .map(|index| RulerMajor {
            index,
            px: to_px(index_value(index, step)),
        })
        .collect();
    #[allow(clippy::cast_precision_loss)]
    let minors = (first..last)
        .flat_map(|index| {
            (1..MINORS_PER_MAJOR).map(move |part| {
                (index as f64 + f64::from(part) / f64::from(MINORS_PER_MAJOR)) * step
            })
        })
        .map(&to_px)
        .collect();

    let label_every = MANTISSAS
        .iter()
        .copied()
        .find(|&every| major_px * f64::from(every) >= widest + LABEL_GAP_PX)
        .unwrap_or(5);
    let labels = majors
        .iter()
        .filter(|major| major.index.rem_euclid(i64::from(label_every)) == 0)
        .filter_map(|major| {
            let text = label_text(major.index, mantissa, exponent);
            #[allow(clippy::cast_precision_loss)]
            let width = text.chars().count() as f64 * digit_px;
            let label_start = major.px + LABEL_OFFSET_PX;
            (label_start >= 0.0 && label_start + width <= length_px).then_some(RulerLabel {
                tick_px: major.px,
                text,
            })
        })
        .collect();

    let origin = to_px(0.0);
    RulerLayout {
        mantissa,
        exponent,
        major_px,
        majors,
        minors,
        label_every,
        labels,
        origin_px: (-ORIGIN_TOLERANCE_PX..=length_px + ORIGIN_TOLERANCE_PX)
            .contains(&origin)
            .then_some(origin),
    }
}

#[allow(clippy::cast_precision_loss)]
fn index_value(index: i64, step: f64) -> f64 {
    index as f64 * step
}

/// The step a layout is built on.
struct Step {
    mantissa: u32,
    exponent: i32,
    /// The first and last major index to draw (up to one step outside the
    /// strip, so the minors next to its edges exist).
    first: i64,
    last: i64,
    /// The width in px of the widest label that can be drawn.
    widest: f64,
}

/// Picks the smallest step that meets criterion 5 for the visible values
/// `start..end` (in the display unit). The widest label is the longest text of
/// any tick whose label could be drawn, not only those at the edges: "0.5"
/// steps give ".5" labels one character longer than their neighbours. `None`
/// when no step fits (a zoom outside any real range) or the strip would hold
/// more than [`MAX_MAJORS`] ticks.
fn choose_step(px_per_unit: f64, start: f64, end: f64, digit_px: f64) -> Option<Step> {
    for exponent in MIN_EXPONENT..=MAX_EXPONENT {
        for mantissa in MANTISSAS {
            let step = f64::from(mantissa) * 10f64.powi(exponent);
            let major_px = step * px_per_unit;
            if major_px < MIN_MAJOR_PX {
                continue;
            }
            #[allow(clippy::cast_possible_truncation)]
            let (first, last) = ((start / step).floor() as i64, (end / step).ceil() as i64);
            if i128::from(last) - i128::from(first) > i128::from(MAX_MAJORS) {
                return None;
            }
            // Ticks whose label starts inside the strip: from LABEL_OFFSET_PX
            // before its start to its end. A tick just outside them cannot
            // carry a drawn label and must not widen the step.
            #[allow(clippy::cast_possible_truncation)]
            let (labelled_first, labelled_last) = (
                ((start - LABEL_OFFSET_PX / px_per_unit) / step).ceil() as i64,
                (end / step).floor() as i64,
            );
            #[allow(clippy::cast_precision_loss)]
            let widest = (labelled_first..=labelled_last)
                .map(|index| label_text(index, mantissa, exponent).chars().count())
                .max()
                .unwrap_or(1) as f64
                * digit_px;
            if major_px >= widest + STEP_LABEL_GAP_PX {
                return Some(Step {
                    mantissa,
                    exponent,
                    first,
                    last,
                    widest,
                });
            }
        }
    }
    None
}

/// The label of major tick `index` of a step of `mantissa * 10^exponent`: the
/// exact decimal of `index * mantissa * 10^exponent`, built from integers.
fn label_text(index: i64, mantissa: u32, exponent: i32) -> String {
    let scaled = i128::from(index) * i128::from(mantissa);
    let digits = scaled.unsigned_abs().to_string();
    let mut text = String::new();
    if scaled < 0 {
        text.push(MINUS);
    }
    if scaled == 0 {
        text.push('0');
    } else if exponent >= 0 {
        text.push_str(&digits);
        text.extend(std::iter::repeat_n('0', exponent.unsigned_abs() as usize));
    } else {
        let decimals = exponent.unsigned_abs() as usize;
        let padded = format!("{digits:0>width$}", width = decimals + 1);
        let (whole, fraction) = padded.split_at(padded.len() - decimals);
        let fraction = fraction.trim_end_matches('0');
        text.push_str(whole);
        if !fraction.is_empty() {
            text.push('.');
            text.push_str(fraction);
        }
    }
    text
}

#[cfg(test)]
mod tests;
