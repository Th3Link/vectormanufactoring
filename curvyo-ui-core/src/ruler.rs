//! The tick layout of one ruler: where the major and minor ticks and the labels
//! of a ruler strip go for a given view, axis, strip length and display unit
//! (`specs/0015-document-size-and-rulers/` criteria 3 to 7, `adrs.md`
//! decision 9).
//!
//! Pure arithmetic. Labels are built from integers (`index * mantissa`, with
//! the decimal point placed by the exponent), never by printing an `f64`, so
//! the tick at 0.3 reads "0.3" and no label has trailing zeros.

use curvyo_document_core::{DisplayUnit, Point, ViewTransform};

/// The smallest on-screen distance between two major ticks, px.
pub const MIN_MAJOR_PX: f64 = 40.0;

/// How far right of (or below) its tick a label starts, px.
pub const LABEL_OFFSET_PX: f64 = 4.0;

/// The room kept between the widest label and the next major tick when the
/// step is chosen, px.
const STEP_LABEL_GAP_PX: f64 = 4.0;

/// The room kept between two drawn labels, px: when the major spacing is
/// smaller than the widest label plus this, labels go on every second, then
/// every fifth major tick.
const LABEL_GAP_PX: f64 = 8.0;

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
    /// The label starts [`LABEL_OFFSET_PX`] after it.
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
    let mm_per_unit = unit.mm_per_unit();
    let px_per_unit = view.scale() * mm_per_unit;
    if !(length_px > 0.0 && px_per_unit.is_finite() && px_per_unit > 0.0) {
        return RulerLayout::empty();
    }
    let along = |point: Point| match axis {
        RulerAxis::Horizontal => point.x,
        RulerAxis::Vertical => point.y,
    };
    let to_px = |value: f64| {
        let mm = value * mm_per_unit;
        let (x, y) = view.document_to_screen(Point::new(mm, mm));
        match axis {
            RulerAxis::Horizontal => x,
            RulerAxis::Vertical => y,
        }
    };
    let from_px = |px: f64| along(view.screen_to_document(px, px)) / mm_per_unit;
    let (start, end) = (from_px(0.0), from_px(length_px));

    let (mantissa, exponent, first, last, widest) = choose_step(px_per_unit, start, end, digit_px);
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
        origin_px: (-1.0..=length_px + 1.0).contains(&origin).then_some(origin),
    }
}

#[allow(clippy::cast_precision_loss)]
fn index_value(index: i64, step: f64) -> f64 {
    index as f64 * step
}

/// Picks the smallest step that meets criterion 5 for the visible values
/// `start..end` (in the display unit). Returns its mantissa and exponent, the
/// first and last major index to draw, and the width in px of the widest
/// visible label (the longest text of any visible tick, not only the edges:
/// "0.5" steps give ".5" labels one character longer than their neighbours).
fn choose_step(px_per_unit: f64, start: f64, end: f64, digit_px: f64) -> (u32, i32, i64, i64, f64) {
    let mut chosen = (5, MAX_EXPONENT, 0, 0, 0.0);
    for exponent in MIN_EXPONENT..=MAX_EXPONENT {
        for mantissa in MANTISSAS {
            let step = f64::from(mantissa) * 10f64.powi(exponent);
            let major_px = step * px_per_unit;
            #[allow(clippy::cast_possible_truncation)]
            let (first, last) = ((start / step).floor() as i64, (end / step).ceil() as i64);
            if major_px < MIN_MAJOR_PX {
                chosen = (mantissa, exponent, first, last, 0.0);
                continue;
            }
            // At least 40 px apart, so a strip holds only a few dozen ticks.
            #[allow(clippy::cast_precision_loss)]
            let widest = (first..=last)
                .map(|index| label_text(index, mantissa, exponent).chars().count())
                .max()
                .unwrap_or(1) as f64
                * digit_px;
            chosen = (mantissa, exponent, first, last, widest);
            if major_px >= widest + STEP_LABEL_GAP_PX {
                return chosen;
            }
        }
    }
    chosen
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
mod tests {
    use super::*;
    use crate::{PX_PER_MM_AT_100, Viewport};

    const DIGIT: f64 = 7.0;

    /// A view at `percent` zoom with the document point `(x_mm, y_mm)` at
    /// screen pixel (0, 0).
    fn view_at(percent: f64, x_mm: f64, y_mm: f64) -> ViewTransform {
        ViewTransform::new(PX_PER_MM_AT_100 * percent / 100.0, Point::new(x_mm, y_mm))
    }

    fn layout(view: ViewTransform, unit: DisplayUnit) -> RulerLayout {
        ruler_layout(view, RulerAxis::Horizontal, 800.0, unit, DIGIT)
    }

    /// Criterion 5's examples in mm: 100 % gives 20 mm, 8000 % gives 0.2 mm,
    /// 2 % gives 1000 mm.
    #[test]
    fn the_spec_examples_pick_the_spec_steps_in_mm() {
        for (percent, step) in [(100.0, 20.0), (8000.0, 0.2), (2.0, 1000.0)] {
            let layout = layout(view_at(percent, 0.0, 0.0), DisplayUnit::Mm);
            assert!(
                (layout.step() - step).abs() < 1e-9,
                "{percent}: {}",
                layout.step()
            );
        }
    }

    /// Criterion 34: at 100 % the major step is 2 cm and 0.5 in.
    #[test]
    fn the_major_step_at_100_percent_is_2_cm_and_half_an_inch() {
        let cm = layout(view_at(100.0, 0.0, 0.0), DisplayUnit::Cm);
        assert!((cm.step() - 2.0).abs() < 1e-9);
        let inches = layout(view_at(100.0, 0.0, 0.0), DisplayUnit::In);
        assert!((inches.step() - 0.5).abs() < 1e-9);
    }

    /// Criterion 5 over every zoom from 2 % to 8000 %: the step is 1, 2 or 5
    /// times a power of ten, the spacing is at least 40 px, and with a view
    /// near the origin (short labels) it is under 100 px.
    #[test]
    fn the_spacing_is_between_40_and_100_px_at_every_zoom_and_unit() {
        for unit in DisplayUnit::ALL {
            for i in 0..=200 {
                let percent = 2.0 * (8000.0_f64 / 2.0).powf(f64::from(i) / 200.0);
                let layout = layout(view_at(percent, -20.0, -20.0), unit);
                assert!(
                    layout.major_px >= MIN_MAJOR_PX && layout.major_px < 100.0,
                    "{unit:?} {percent}: {}",
                    layout.major_px
                );
                assert!(MANTISSAS.contains(&layout.mantissa));
                assert_eq!(layout.minors.len(), (layout.majors.len() - 1) * 4);
            }
        }
    }

    /// A wide label raises the step: the spacing is at least the widest
    /// label plus 4 px.
    #[test]
    fn the_spacing_leaves_room_for_the_widest_label() {
        // 100 % at 987654 mm: labels such as "987680" are six characters.
        let view = view_at(100.0, 987_654.0, 0.0);
        let layout = layout(view, DisplayUnit::Mm);
        assert!(layout.major_px >= 6.0 * DIGIT + 4.0, "{}", layout.major_px);
    }

    /// Criterion 6: labels are exact decimals without trailing zeros.
    #[test]
    fn labels_are_exact_decimals_built_from_integers() {
        assert_eq!(label_text(3, 1, -1), "0.3");
        assert_eq!(label_text(-3, 1, -1), "\u{2212}0.3");
        assert_eq!(label_text(20, 1, -1), "2");
        assert_eq!(label_text(0, 5, -3), "0");
        assert_eq!(label_text(7, 5, -2), "0.35");
        assert_eq!(label_text(3, 2, 1), "60");
        assert_eq!(label_text(-5, 1, 2), "\u{2212}500");
        assert_eq!(label_text(1, 2, -3), "0.002");
        assert_eq!(label_text(12, 5, -4), "0.006");
        assert_eq!(label_text(1_000_000, 1, 0), "1000000");
        assert_eq!(label_text(3, 1, -6), "0.000003");
    }

    /// Criterion 6 at every zoom: only digits, one optional point, a leading
    /// minus sign; no trailing zero after a point; the tick at 0.3 reads "0.3".
    #[test]
    fn no_label_at_any_zoom_has_float_noise_or_trailing_zeros() {
        for unit in DisplayUnit::ALL {
            for i in 0..=100 {
                let percent = 2.0 * (4000.0_f64).powf(f64::from(i) / 100.0);
                for x in [-300.0, -1.7, 0.0, 33.3, 12_345.6] {
                    for label in &layout(view_at(percent, x, x), unit).labels {
                        let text = &label.text;
                        let body = text.strip_prefix(MINUS).unwrap_or(text);
                        assert!(
                            body.chars().all(|c| c.is_ascii_digit() || c == '.'),
                            "{text}"
                        );
                        assert!(body.matches('.').count() <= 1, "{text}");
                        if body.contains('.') {
                            assert!(!body.ends_with('0') && !body.ends_with('.'), "{text}");
                        }
                    }
                }
            }
        }
        let fine = layout(view_at(8000.0, 0.0, 0.0), DisplayUnit::Mm);
        assert!(
            fine.labels.iter().any(|label| label.text == "0.4"),
            "{fine:?}"
        );
    }

    /// Criterion 3: value 0 is at the document's top-left corner on both
    /// axes, values grow right and down.
    #[test]
    fn the_origin_tick_is_at_the_document_corner_on_both_axes() {
        let view = view_at(100.0, -10.0, -20.0);
        let horizontal = ruler_layout(view, RulerAxis::Horizontal, 800.0, DisplayUnit::Mm, DIGIT);
        let vertical = ruler_layout(view, RulerAxis::Vertical, 600.0, DisplayUnit::Mm, DIGIT);
        let corner = view.document_to_screen(Point::new(0.0, 0.0));
        assert!((horizontal.origin_px.unwrap() - corner.0).abs() < 1e-9);
        assert!((vertical.origin_px.unwrap() - corner.1).abs() < 1e-9);
        // Growing right: the tick after 0 is further along.
        let after = horizontal.majors.iter().find(|m| m.index == 1).unwrap();
        assert!(after.px > horizontal.origin_px.unwrap());
        let after = vertical.majors.iter().find(|m| m.index == 1).unwrap();
        assert!(after.px > vertical.origin_px.unwrap());
    }

    /// Criterion 4: left of and above the corner the values are negative,
    /// past the document's size they continue; there is no gap.
    #[test]
    fn the_ruler_continues_on_the_pasteboard_without_a_break() {
        let view = view_at(100.0, -150.0, -150.0);
        let layout = layout(view, DisplayUnit::Mm);
        assert!(layout.majors.iter().any(|m| m.index < 0));
        assert!(layout.labels.iter().any(|l| l.text.starts_with(MINUS)));
        let right = self::layout(view_at(100.0, 100.0, 100.0), DisplayUnit::Mm);
        // Past 210 mm (A4 width): 100 % shows 100 mm to 311 mm here.
        assert!(
            right.labels.iter().any(|l| l.text == "240"),
            "{:?}",
            right.labels
        );
        let step = layout.major_px;
        for pair in layout.majors.windows(2) {
            assert_eq!(pair[1].index, pair[0].index + 1);
            assert!((pair[1].px - pair[0].px - step).abs() < 1e-6);
        }
        let first = layout.majors.first().unwrap();
        let last = layout.majors.last().unwrap();
        assert!(first.px <= 0.0 && last.px >= 800.0);
    }

    /// Criterion 2: after each of 20 pan and zoom steps every major tick lies
    /// where the canvas projects its document value, within 0.5 px.
    #[test]
    fn every_major_tick_matches_the_canvas_projection_after_pan_and_zoom() {
        let mut viewport = Viewport::new();
        viewport.resize(800.0, 600.0);
        for step in 0..20_u32 {
            match step % 4 {
                0 => viewport.pan_by_screen_delta(37.0 * f64::from(step), -23.0),
                1 => viewport.zoom_about(300.0, 200.0, 1.7),
                2 => viewport.pan_by_screen_delta(-410.0, 91.0),
                _ => viewport.zoom_about(10.0, 590.0, 0.45),
            }
            let view = viewport.view();
            for unit in DisplayUnit::ALL {
                let layout = layout(view, unit);
                for axis in [RulerAxis::Horizontal, RulerAxis::Vertical] {
                    let layout = ruler_layout(view, axis, 600.0, unit, DIGIT);
                    for major in &layout.majors {
                        #[allow(clippy::cast_precision_loss)]
                        let mm = major.index as f64 * layout.step() * unit.mm_per_unit();
                        let (x, y) = view.document_to_screen(Point::new(mm, mm));
                        let projected = if axis == RulerAxis::Horizontal { x } else { y };
                        assert!((major.px - projected).abs() < 0.5, "step {step}");
                    }
                }
                assert!(layout.majors.len() > 1);
            }
        }
    }

    /// Criterion 7: at any zoom and for values up to 1,000,000 in the display
    /// unit every drawn label lies inside the strip and no two labels touch.
    #[test]
    fn no_label_is_clipped_or_overlaps_another_up_to_a_million() {
        for unit in DisplayUnit::ALL {
            let to_mm = unit.mm_per_unit();
            for i in 0..=40 {
                let percent = 2.0 * (4000.0_f64).powf(f64::from(i) / 40.0);
                for centre in [
                    -1_000_000.0,
                    -999_999.9,
                    -5.5,
                    0.0,
                    3.0,
                    999_999.9,
                    1_000_000.0,
                ] {
                    let scale = PX_PER_MM_AT_100 * percent / 100.0;
                    // The view whose strip is centred on `centre` in the unit.
                    let origin = centre * to_mm - 400.0 / scale;
                    let view = ViewTransform::new(scale, Point::new(origin, origin));
                    for length in [90.0, 400.0, 800.0] {
                        let layout = ruler_layout(view, RulerAxis::Horizontal, length, unit, DIGIT);
                        let mut previous_end = f64::NEG_INFINITY;
                        for label in &layout.labels {
                            #[allow(clippy::cast_precision_loss)]
                            let width = label.text.chars().count() as f64 * DIGIT;
                            let start = label.tick_px + LABEL_OFFSET_PX;
                            assert!(start >= 0.0 && start + width <= length, "{label:?}");
                            assert!(start >= previous_end, "overlap at {percent} {centre}");
                            previous_end = start + width;
                        }
                    }
                }
            }
        }
    }

    /// Criterion 7: labels wider than the spacing allows go on every second
    /// tick, and ticks are never removed.
    #[test]
    fn labels_thin_out_to_every_second_tick_when_they_would_touch() {
        // 4.2 px per mm: the 10 mm step is 42 px, the labels "200" are 36 px
        // wide at 12 px per digit: they fit the step (40) but not with 8 px
        // between them (44).
        let view = ViewTransform::new(4.2, Point::new(0.0, 0.0));
        let layout = ruler_layout(view, RulerAxis::Horizontal, 800.0, DisplayUnit::Mm, 12.0);
        assert!((layout.step() - 10.0).abs() < 1e-9);
        assert_eq!(layout.label_every, 2);
        assert!(layout.labels.iter().all(|l| {
            let major = layout
                .majors
                .iter()
                .find(|m| (m.px - l.tick_px).abs() < 1e-9)
                .unwrap();
            major.index % 2 == 0
        }));
        assert!(
            layout
                .majors
                .windows(2)
                .all(|p| p[1].index == p[0].index + 1)
        );
    }

    #[test]
    fn a_label_that_does_not_fit_in_the_strip_is_not_drawn() {
        // The tick at 0 sits 2 px from the strip's start: its label would
        // start at 6 px and be 7 px wide, so with a 10 px strip it is cut.
        let view = ViewTransform::new(PX_PER_MM_AT_100, Point::new(-2.0 / PX_PER_MM_AT_100, 0.0));
        let layout = ruler_layout(view, RulerAxis::Horizontal, 10.0, DisplayUnit::Mm, DIGIT);
        assert!(layout.labels.is_empty(), "{:?}", layout.labels);
        assert!(!layout.majors.is_empty(), "ticks are never removed");
    }

    #[test]
    fn an_empty_strip_has_an_empty_layout() {
        let view = view_at(100.0, 0.0, 0.0);
        let layout = ruler_layout(view, RulerAxis::Horizontal, 0.0, DisplayUnit::Mm, DIGIT);
        assert!(layout.majors.is_empty() && layout.labels.is_empty());
    }
}
