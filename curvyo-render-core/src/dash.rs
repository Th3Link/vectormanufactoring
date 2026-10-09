//! Turning an outline into the on-intervals of a dash pattern
//! (`specs/0007-stroke-and-fill-styling/specification.md`, criteria 7 to 9),
//! with the guards that keep a crafted pattern or width from exhausting memory
//! or time (`adrs.md`, readiness check section 5).

use std::borrow::Cow;

use curvyo_document_core::{DashPattern, LineCap};
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

/// The pattern as the renderer walks it: an odd list repeats once, so `1 2 4`
/// draws as `1 2 4 1 2 4` and the second round starts with an off gap, as SVG
/// defines it (`specs/0017-style-panel-rework`, criterion 32).
fn walked(pattern: &DashPattern) -> Cow<'_, [f64]> {
    let ratios = pattern.as_slice();
    if ratios.len() % 2 == 1 {
        Cow::Owned([ratios, ratios].concat())
    } else {
        Cow::Borrowed(ratios)
    }
}

/// The dashed version of each outline of **one object**: one open sub-path per on-interval,
/// so each dash is stroked with its own caps (acceptance criterion 12). `None` means "draw it
/// solid": a solid pattern, a period under [`MIN_DASH_PERIOD_PX`] on screen, outlines of no
/// length, or a pattern that would pass the per-object or per-frame dash limit. `width_mm` is the
/// **document** stroke width, which the pattern is a multiple of; `scale` is screen pixels per
/// millimetre. A zero-length "on" entry makes a dot when `cap` is round or square (and nothing with
/// a butt cap), as SVG draws it.
///
/// Each outline is dashed on
/// its own so the pattern starts afresh at its first node (criterion 31a of
/// `specs/0016-boolean-operations`). The limits are the object's, not each
/// outline's: the dashes of all outlines together must stay within
/// [`MAX_DASHES_PER_OBJECT`] and the frame's remaining budget, otherwise `None`
/// and the whole object is drawn solid, so no outline of it is dashed while
/// another is not. An outline of no length is returned as it is.
// The counts are small (bounded by the dash limits) and the geometry is `f32`
// in `lyon`; the casts are the unit changes between them.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub(crate) fn dashed_object(
    outlines: &[Path],
    pattern: &DashPattern,
    cap: LineCap,
    width_mm: f64,
    scale: f64,
    tolerance_mm: f64,
    budget: &mut DashBudget,
) -> Option<Vec<Path>> {
    let ratios = walked(pattern);
    let ratios = ratios.as_ref();
    // A width past the display cap draws solid, like the stroke it belongs to:
    // `ratio * width` would overflow `f32` in the tessellator.
    if ratios.is_empty()
        || !width_mm.is_finite()
        || width_mm <= 0.0
        || width_mm > crate::artwork::MAX_DISPLAY_STROKE_WIDTH_MM
    {
        return None;
    }
    let period_mm = ratios.iter().sum::<f64>() * width_mm;
    if !period_mm.is_finite() || period_mm * scale < MIN_DASH_PERIOD_PX {
        return None;
    }
    let dots = cap != LineCap::Butt;
    let on_intervals = ratios
        .chunks(2)
        .filter(|pair| pair[0] > 0.0 || dots)
        .count();
    let measured: Vec<(PathMeasurements, f64)> = outlines
        .iter()
        .map(|path| {
            let measurements = PathMeasurements::from_path(path, tolerance_mm as f32);
            let length = f64::from(measurements.length());
            (measurements, length)
        })
        .collect();
    if measured
        .iter()
        .any(|(_, length)| !length.is_finite() || *length < 0.0)
        || measured.iter().all(|(_, length)| *length <= 0.0)
    {
        return None;
    }
    let estimate: f64 = measured
        .iter()
        .filter(|(_, length)| *length > 0.0)
        .map(|(_, length)| (length / period_mm).ceil() * on_intervals as f64)
        .sum();
    if estimate > MAX_DASHES_PER_OBJECT as f64 || estimate > budget.remaining as f64 {
        return None;
    }
    let walk = Walk {
        ratios,
        width: width_mm as f32,
        period_mm,
        on_intervals,
        dots,
    };
    let mut drawn = 0_usize;
    let dashed: Vec<Path> = outlines
        .iter()
        .zip(&measured)
        .map(|(path, (measurements, length))| {
            if *length <= 0.0 {
                return path.clone();
            }
            let (dashes, count) = dash_outline(path, measurements, *length, &walk);
            drawn += count;
            dashes
        })
        .collect();
    budget.remaining = budget.remaining.saturating_sub(drawn);
    Some(dashed)
}

/// What walking one object's pattern needs, the same for each of its outlines.
struct Walk<'a> {
    /// The pattern as walked (an odd list already doubled).
    ratios: &'a [f64],
    /// The document stroke width, millimetres, as `lyon` takes it.
    width: f32,
    /// The length of one round of the pattern, millimetres.
    period_mm: f64,
    /// How many "on" entries a round has that draw something.
    on_intervals: usize,
    /// A zero-length "on" entry draws a dot (a round or square cap).
    dots: bool,
}

/// The dashes of one outline of positive `length`, and how many there are. The estimate of the
/// dash count bounds the loop; the explicit step cap also stops it if float rounding ever failed
/// to advance the distance.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn dash_outline(
    path: &Path,
    measurements: &PathMeasurements,
    length: f64,
    walk: &Walk<'_>,
) -> (Path, usize) {
    let Walk {
        ratios,
        width,
        period_mm,
        on_intervals,
        dots,
    } = *walk;
    let length = length as f32;
    let outline_estimate = (f64::from(length) / period_mm).ceil() * on_intervals as f64;
    let mut sampler = measurements.create_sampler(path, SampleType::Distance);
    let mut builder = Path::builder();
    let mut distance = 0.0_f32;
    let mut drawn = 0_usize;
    let max_steps = ratios.len() * (outline_estimate as usize + 2);
    for step in 0..max_steps {
        if distance >= length {
            break;
        }
        let run = ratios[step % ratios.len()] as f32 * width;
        if step % 2 == 0 {
            if run > 0.0 {
                sampler.split_range(distance..distance + run, &mut builder);
                drawn += 1;
            } else if dots {
                // A zero-length dash: a segment of a thousandth of the stroke width along
                // the outline's direction there, so a round or square cap draws the dot and
                // the square follows the line. (`lyon` draws no cap on a sub-path of no
                // length at all.)
                let at = sampler.sample(distance).position();
                // The direction from two points on the outline, not the sampler's tangent,
                // which is not a number at the end of a degenerate curve (a corner anchor's
                // zero handles).
                let step = (width * 0.01).max(f32::EPSILON * length).min(length);
                let ahead = sampler.sample((distance + step).min(length)).position();
                let behind = sampler.sample((distance - step).max(0.0)).position();
                let along = (ahead - behind).normalize();
                if along.x.is_finite() && along.y.is_finite() {
                    let half = along * (width * 0.001);
                    builder.begin(at - half);
                    builder.line_to(at + half);
                    builder.end(false);
                    drawn += 1;
                }
            }
        }
        distance += run;
    }
    (builder.build(), drawn)
}

#[cfg(test)]
mod tests {
    use lyon::math::point;

    use super::*;

    /// One path dashed alone: `dashed_object` of a single outline.
    fn dashed(
        path: &Path,
        pattern: &DashPattern,
        width_mm: f64,
        scale: f64,
        tolerance_mm: f64,
        budget: &mut DashBudget,
    ) -> Option<Path> {
        dashed_object(
            std::slice::from_ref(path),
            pattern,
            LineCap::Butt,
            width_mm,
            scale,
            tolerance_mm,
            budget,
        )?
        .pop()
    }

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

    /// Every point of every event, in order: two paths with the same points
    /// were dashed the same way.
    fn points(path: &Path) -> Vec<[f32; 2]> {
        path.iter()
            .flat_map(|event| match event {
                lyon::path::Event::Begin { at } => vec![[at.x, at.y]],
                lyon::path::Event::Line { to, .. } => vec![[to.x, to.y]],
                _ => Vec::new(),
            })
            .collect()
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
    fn a_zero_on_entry_is_skipped_with_a_butt_cap() {
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

    fn dashed_with_cap(lengths: &[f64], cap: LineCap, length: f32, width: f64) -> Option<Path> {
        dashed_object(
            &[line(length)],
            &pattern(lengths),
            cap,
            width,
            10.0,
            0.01,
            &mut DashBudget::per_frame(),
        )?
        .pop()
    }

    #[test]
    fn a_zero_on_entry_makes_a_dot_with_a_round_or_square_cap() {
        for cap in [LineCap::Round, LineCap::Square] {
            // `0 3` at width 1: a dot every 3 mm, on a 30 mm line.
            let path = dashed_with_cap(&[0.0, 3.0], cap, 30.0, 1.0).expect("dashed");
            assert_eq!(subpaths(&path), 10, "{cap:?}");
        }
        let butt = dashed_with_cap(&[0.0, 3.0], LineCap::Butt, 30.0, 1.0).expect("dashed");
        assert_eq!(
            subpaths(&butt),
            0,
            "a butt cap draws nothing for a zero dash"
        );
    }

    #[test]
    fn a_dot_sits_on_the_outline_at_its_distance() {
        let path = dashed_with_cap(&[0.0, 5.0], LineCap::Round, 12.0, 1.0).expect("dashed");
        let xs: Vec<f32> = path
            .iter()
            .filter_map(|event| match event {
                lyon::path::Event::Begin { at } => Some(at.x),
                _ => None,
            })
            .collect();
        // Dots at 0, 5 and 10 mm (each starts a thousandth of a width before its point).
        assert_eq!(xs.len(), 3);
        for (x, want) in xs.iter().zip([0.0_f32, 5.0, 10.0]) {
            assert!((x - want).abs() < 0.01, "{x} vs {want}");
        }
    }

    #[test]
    fn an_odd_list_is_walked_twice() {
        // `1 2 4` at width 1 walks as `1 2 4 1 2 4`: on 1, off 2, on 4, off 1, on 2, off 4:
        // period 14 mm, 3 dashes per period, a 28 mm line has 6 dashes.
        let odd = dashed_with_cap(&[1.0, 2.0, 4.0], LineCap::Butt, 28.0, 1.0).expect("dashed");
        let even = dashed_with_cap(&[1.0, 2.0, 4.0, 1.0, 2.0, 4.0], LineCap::Butt, 28.0, 1.0)
            .expect("dashed");
        assert_eq!(subpaths(&odd), 6);
        assert_eq!(
            points(&odd),
            points(&even),
            "an odd list draws exactly as its doubling"
        );
    }

    #[test]
    fn a_single_number_is_on_then_the_same_off() {
        // `4` walks as `4 4`.
        let one = dashed_with_cap(&[4.0], LineCap::Butt, 32.0, 1.0).expect("dashed");
        let two = dashed_with_cap(&[4.0, 4.0], LineCap::Butt, 32.0, 1.0).expect("dashed");
        assert_eq!(points(&one), points(&two));
        assert_eq!(subpaths(&one), 4);
    }

    #[test]
    fn a_width_past_the_display_cap_draws_solid() {
        let mut budget = DashBudget::per_frame();
        let p = pattern(&[6.0, 4.0]);
        let cap = crate::artwork::MAX_DISPLAY_STROKE_WIDTH_MM;
        assert!(dashed(&line(1e9), &p, cap * 2.0, 1e-9, 1.0, &mut budget).is_none());
    }
}
