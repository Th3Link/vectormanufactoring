//! Walking one outline with a dash pattern: the on-intervals as open sub-paths,
//! and the dots a zero-length "on" entry draws
//! (`specs/0007-stroke-and-fill-styling` criteria 7 to 9, `specs/0017-style-panel-
//! rework` criterion 32). The limits that decide whether to dash at all are
//! `dash.rs`.

use std::borrow::Cow;

use curvyo_document_core::DashPattern;
use lyon::algorithms::measure::{PathMeasurements, PathSampler, SampleType};
use lyon::path::Path;

/// The pattern as the renderer walks it: an odd list repeats once, so `1 2 4`
/// draws as `1 2 4 1 2 4` and the second round starts with an off gap, as SVG
/// defines it (`specs/0017-style-panel-rework`, criterion 32).
pub(super) fn walked(pattern: &DashPattern) -> Cow<'_, [f64]> {
    let ratios = pattern.as_slice();
    if ratios.len() % 2 == 1 {
        Cow::Owned([ratios, ratios].concat())
    } else {
        Cow::Borrowed(ratios)
    }
}

/// What walking one object's pattern needs, the same for each of its outlines.
pub(super) struct Walk<'a> {
    /// The pattern as walked (an odd list already doubled).
    pub(super) ratios: &'a [f64],
    /// The document stroke width, millimetres, as `lyon` takes it.
    pub(super) width: f32,
    /// The length of one round of the pattern, millimetres.
    pub(super) period_mm: f64,
    /// How many "on" entries a round has that draw something.
    pub(super) on_intervals: usize,
    /// A zero-length "on" entry draws a dot (a round or square cap).
    pub(super) dots: bool,
}

/// The dashes of one outline of positive `length`, and how many there are. The estimate of the
/// dash count bounds the loop; the explicit step cap also stops it if float rounding ever failed
/// to advance the distance.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub(super) fn dash_outline(
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
                drawn += usize::from(add_dot(&mut sampler, distance, length, width, &mut builder));
            }
        }
        distance += run;
    }
    (builder.build(), drawn)
}

/// A zero-length dash at `distance`: a segment of two thousandths of the stroke
/// width (a thousandth either side) along the outline's direction there, so a
/// round or square cap draws the dot and the square follows the line. (`lyon`
/// draws no cap on a sub-path of no length at all.) The direction comes from two
/// points on the outline, not the sampler's tangent, which is not a number at
/// the end of a degenerate curve (a corner anchor's zero handles). Returns
/// whether a dot was added.
fn add_dot(
    sampler: &mut PathSampler<'_, Path, ()>,
    distance: f32,
    length: f32,
    width: f32,
    builder: &mut lyon::path::path::Builder,
) -> bool {
    let at = sampler.sample(distance).position();
    let reach = (width * 0.01).max(f32::EPSILON * length).min(length);
    let ahead = sampler.sample((distance + reach).min(length)).position();
    let behind = sampler.sample((distance - reach).max(0.0)).position();
    let along = (ahead - behind).normalize();
    if !(along.x.is_finite() && along.y.is_finite()) {
        return false;
    }
    let half = along * (width * 0.001);
    builder.begin(at - half);
    builder.line_to(at + half);
    builder.end(false);
    true
}
