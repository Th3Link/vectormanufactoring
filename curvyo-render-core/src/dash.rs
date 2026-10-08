//! Turning an outline into the on-intervals of a dash pattern
//! (`specs/0007-stroke-and-fill-styling/specification.md`, criteria 7 to 9),
//! with the guards that keep a crafted pattern or width from exhausting memory
//! or time (`adrs.md`, readiness check section 5).

use curvyo_document_core::DashPattern;
use lyon::algorithms::measure::{PathMeasurements, SampleType};
use lyon::path::Path;

/// The most dashes one object may draw. A pattern that would produce more
/// draws solid instead.
pub(crate) const MAX_DASHES_PER_OBJECT: usize = 2000;

/// The most dashes one frame may draw across all objects. Once spent, further
/// dashed objects draw solid.
pub(crate) const MAX_DASHES_PER_FRAME: usize = 50_000;

/// A pattern whose period (every on and off length) is shorter than this on
/// screen draws solid: it would be a grey smear, and the vertex count would
/// be wasted (acceptance criterion 8: that is the display, not a stored
/// change).
pub(crate) const MIN_DASH_PERIOD_PX: f64 = 2.0;

/// The dashes left for the rest of one frame.
#[derive(Debug)]
pub(crate) struct DashBudget {
    remaining: usize,
}

impl DashBudget {
    pub(crate) const fn per_frame() -> Self {
        Self {
            remaining: MAX_DASHES_PER_FRAME,
        }
    }
}

/// The dashed version of `path`: one open sub-path per on-interval, so each
/// dash is stroked with its own caps (acceptance criterion 12). `None` means
/// "draw it solid": a solid pattern, a period under [`MIN_DASH_PERIOD_PX`] on
/// screen, a path of no length, or a pattern that would pass the per-object or
/// per-frame dash limit.
///
/// `width_mm` is the **document** stroke width, which the pattern is a
/// multiple of; the drawn width may be floored for display and does not enter
/// here. `scale` is screen pixels per millimetre.
// The counts are small (bounded by the dash limits just above the casts) and
// the geometry is `f32` in `lyon`; the casts are the unit changes between them.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub(crate) fn dashed(
    path: &Path,
    pattern: &DashPattern,
    width_mm: f64,
    scale: f64,
    tolerance_mm: f64,
    budget: &mut DashBudget,
) -> Option<Path> {
    let ratios = pattern.as_slice();
    if ratios.is_empty() || !width_mm.is_finite() || width_mm <= 0.0 {
        return None;
    }
    let period_mm = ratios.iter().sum::<f64>() * width_mm;
    if !period_mm.is_finite() || period_mm * scale < MIN_DASH_PERIOD_PX {
        return None;
    }
    let measurements = PathMeasurements::from_path(path, tolerance_mm as f32);
    let length = f64::from(measurements.length());
    if !length.is_finite() || length <= 0.0 {
        return None;
    }
    let on_intervals = ratios.chunks(2).filter(|pair| pair[0] > 0.0).count();
    let estimate = (length / period_mm).ceil() * on_intervals as f64;
    if estimate > MAX_DASHES_PER_OBJECT as f64 || estimate > budget.remaining as f64 {
        return None;
    }
    let (length, width) = (length as f32, width_mm as f32);
    let mut sampler = measurements.create_sampler(path, SampleType::Distance);
    let mut builder = Path::builder();
    let mut distance = 0.0_f32;
    let mut drawn = 0_usize;
    // The estimate bounds the loop; the explicit cap also stops it if float
    // rounding ever failed to advance `distance`.
    let max_steps = ratios.len() * (estimate as usize + 2);
    for step in 0..max_steps {
        if distance >= length {
            break;
        }
        let run = ratios[step % ratios.len()] as f32 * width;
        if step % 2 == 0 && run > 0.0 {
            sampler.split_range(distance..distance + run, &mut builder);
            drawn += 1;
        }
        distance += run;
    }
    budget.remaining = budget.remaining.saturating_sub(drawn);
    Some(builder.build())
}

#[cfg(test)]
mod tests {
    use lyon::math::point;

    use super::*;

    fn line(length: f32) -> Path {
        let mut builder = Path::builder();
        builder.begin(point(0.0, 0.0));
        builder.line_to(point(length, 0.0));
        builder.end(false);
        builder.build()
    }

    fn pattern(lengths: &[f64]) -> DashPattern {
        DashPattern::new(lengths.to_vec()).unwrap()
    }

    fn subpaths(path: &Path) -> usize {
        path.iter()
            .filter(|event| matches!(event, lyon::path::Event::Begin { .. }))
            .count()
    }

    #[test]
    fn a_dash_dot_pattern_makes_one_subpath_per_on_interval() {
        let mut budget = DashBudget::per_frame();
        // Width 1 mm, pattern [6, 4]: period 10 mm, a 100 mm line has 10 dashes.
        let path = dashed(
            &line(100.0),
            &pattern(&[6.0, 4.0]),
            1.0,
            5.0,
            0.01,
            &mut budget,
        )
        .expect("dashed");
        assert_eq!(subpaths(&path), 10);
    }

    #[test]
    fn the_dash_lengths_are_multiples_of_the_document_width() {
        let mut budget = DashBudget::per_frame();
        let narrow = dashed(
            &line(60.0),
            &pattern(&[6.0, 4.0]),
            0.5,
            20.0,
            0.01,
            &mut budget,
        )
        .expect("dashed");
        let wide = dashed(
            &line(60.0),
            &pattern(&[6.0, 4.0]),
            1.0,
            20.0,
            0.01,
            &mut budget,
        )
        .expect("dashed");
        // Half the width, half the period, twice the dashes.
        assert_eq!(subpaths(&narrow), 2 * subpaths(&wide));
    }

    #[test]
    fn a_period_under_two_screen_pixels_draws_solid() {
        let mut budget = DashBudget::per_frame();
        // Period 4 * 0.25 = 1 mm at 1.5 px/mm is 1.5 px.
        assert!(
            dashed(
                &line(100.0),
                &pattern(&[1.0, 3.0]),
                0.25,
                1.5,
                0.01,
                &mut budget
            )
            .is_none()
        );
        // At 2 px per mm the period is exactly 2 px and dashes are drawn.
        assert!(
            dashed(
                &line(100.0),
                &pattern(&[1.0, 3.0]),
                0.25,
                2.0,
                0.01,
                &mut budget
            )
            .is_some()
        );
    }

    #[test]
    fn a_pattern_past_the_per_object_limit_draws_solid() {
        let mut budget = DashBudget::per_frame();
        // 100 mm / (0.05 mm) = 2000 periods ... 2001 dashes is past the limit.
        let path = line(100.05);
        assert!(
            dashed(
                &path,
                &pattern(&[1.0, 1.0]),
                0.025,
                400.0,
                0.001,
                &mut budget
            )
            .is_none()
        );
        assert!(
            dashed(
                &path,
                &pattern(&[1.0, 1.0]),
                0.05,
                400.0,
                0.001,
                &mut budget
            )
            .is_some()
        );
    }

    #[test]
    fn the_per_frame_budget_is_spent_across_objects() {
        let mut budget = DashBudget { remaining: 25 };
        let p = pattern(&[1.0, 1.0]);
        assert!(dashed(&line(100.0), &p, 2.0, 5.0, 0.01, &mut budget).is_some()); // 25 dashes
        assert!(dashed(&line(100.0), &p, 2.0, 5.0, 0.01, &mut budget).is_none());
    }

    #[test]
    fn extreme_widths_and_empty_paths_draw_solid_without_panicking() {
        let mut budget = DashBudget::per_frame();
        let p = pattern(&[6.0, 4.0]);
        for width in [f64::NAN, f64::INFINITY, 0.0, -1.0, 1e300, 1e-300] {
            let _ = dashed(&line(10.0), &p, width, 5.0, 0.01, &mut budget);
        }
        let empty = Path::builder().build();
        assert!(dashed(&empty, &p, 1.0, 5.0, 0.01, &mut budget).is_none());
        assert!(dashed(&line(0.0), &p, 1.0, 5.0, 0.01, &mut budget).is_none());
        assert!(
            dashed(
                &line(10.0),
                &DashPattern::solid(),
                1.0,
                5.0,
                0.01,
                &mut budget
            )
            .is_none()
        );
    }

    #[test]
    fn a_zero_on_entry_is_skipped() {
        let mut budget = DashBudget::per_frame();
        let path = dashed(
            &line(40.0),
            &pattern(&[0.0, 3.0, 2.0, 3.0]),
            1.0,
            10.0,
            0.01,
            &mut budget,
        )
        .expect("dashed");
        assert_eq!(subpaths(&path), 5);
    }
}
