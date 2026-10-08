//! The colour a gradient has at a position along it
//! (`specs/0007-stroke-and-fill-styling` criteria 16, 18, 35): the one rule the
//! renderer's ramp and the editor's "a new stop gets the colour the ramp has
//! there" share, so adding a stop changes nothing on screen.

use crate::path_model::Color;
use crate::style_model::{GradientStop, Opacity};

/// `stops` in render order: a stable sort by position, so stops at the same
/// position keep the order they have in the list (the maker's, which decides
/// the sides of a hard edge).
#[must_use]
pub fn sorted_stops(stops: &[GradientStop]) -> Vec<GradientStop> {
    let mut sorted = stops.to_vec();
    sorted.sort_by(|a, b| a.position.get().total_cmp(&b.position.get()));
    sorted
}

fn lerp(a: f64, b: f64, k: f64) -> f64 {
    (b - a).mul_add(k, a)
}

fn channel(a: u8, b: u8, k: f64) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let value = lerp(f64::from(a), f64::from(b), k)
        .round()
        .clamp(0.0, 255.0) as u8;
    value
}

/// The colour and opacity of the ramp of `sorted` (stops in render order, see
/// [`sorted_stops`]) at `t`; `None` when there are no stops (nothing is
/// painted). Interpolation runs on the sRGB-encoded channel values and on the
/// opacity, not premultiplied, as SVG's default does. Before the first stop and
/// after the last the colour of that stop continues ("pad"); one stop is a
/// uniform paint. At the position of coincident stops the later one speaks, so
/// the edge is hard.
#[must_use]
pub fn ramp_at(sorted: &[GradientStop], t: f64) -> Option<(Color, Opacity)> {
    let first = sorted.first()?;
    let t = if t.is_finite() { t } else { 0.0 };
    // The first stop strictly after `t`.
    let Some(after) = sorted.iter().position(|stop| stop.position.get() > t) else {
        let last = sorted.last()?;
        return Some((last.color, last.opacity));
    };
    if after == 0 {
        return Some((first.color, first.opacity));
    }
    let (from, to) = (sorted[after - 1], sorted[after]);
    let span = to.position.get() - from.position.get();
    let k = if span > 0.0 {
        ((t - from.position.get()) / span).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let opacity = lerp(from.opacity.get(), to.opacity.get(), k).clamp(0.0, 1.0);
    Some((
        Color {
            r: channel(from.color.r, to.color.r, k),
            g: channel(from.color.g, to.color.g, k),
            b: channel(from.color.b, to.color.b, k),
        },
        Opacity::new(opacity).unwrap_or(Opacity::OPAQUE),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style_model::{StopId, StopPosition};

    fn stop(n: u64, at: f64, r: u8, g: u8, b: u8, alpha: f64) -> GradientStop {
        GradientStop {
            id: StopId::new(1, n),
            position: StopPosition::new(at).unwrap(),
            color: Color { r, g, b },
            opacity: Opacity::new(alpha).unwrap(),
        }
    }

    fn at(stops: &[GradientStop], t: f64) -> (Color, f64) {
        let (color, opacity) = ramp_at(&sorted_stops(stops), t).unwrap();
        (color, opacity.get())
    }

    /// The golden ramp of the default red-to-white gradient: encoded channels
    /// interpolate linearly.
    #[test]
    fn red_to_white_interpolates_the_encoded_channels() {
        let stops = [
            stop(1, 0.0, 255, 0, 0, 1.0),
            stop(2, 1.0, 255, 255, 255, 1.0),
        ];
        assert_eq!(at(&stops, 0.0).0, Color { r: 255, g: 0, b: 0 });
        assert_eq!(
            at(&stops, 0.25).0,
            Color {
                r: 255,
                g: 64,
                b: 64
            }
        );
        assert_eq!(
            at(&stops, 0.5).0,
            Color {
                r: 255,
                g: 128,
                b: 128
            }
        );
        assert_eq!(
            at(&stops, 1.0).0,
            Color {
                r: 255,
                g: 255,
                b: 255
            }
        );
    }

    #[test]
    fn opacity_interpolates_on_its_own_and_not_premultiplied() {
        let stops = [stop(1, 0.0, 0, 0, 0, 1.0), stop(2, 1.0, 255, 255, 255, 0.0)];
        let (color, alpha) = at(&stops, 0.5);
        assert_eq!(
            color,
            Color {
                r: 128,
                g: 128,
                b: 128
            }
        );
        assert!((alpha - 0.5).abs() < 1e-12);
    }

    #[test]
    fn the_end_colours_pad_and_one_stop_is_uniform() {
        let stops = [
            stop(1, 0.25, 10, 20, 30, 1.0),
            stop(2, 0.75, 200, 0, 0, 1.0),
        ];
        assert_eq!(
            at(&stops, -3.0).0,
            Color {
                r: 10,
                g: 20,
                b: 30
            }
        );
        assert_eq!(
            at(&stops, 0.0).0,
            Color {
                r: 10,
                g: 20,
                b: 30
            }
        );
        assert_eq!(at(&stops, 9.0).0, Color { r: 200, g: 0, b: 0 });
        let one = [stop(1, 0.4, 1, 2, 3, 0.5)];
        for t in [0.0, 0.4, 1.0] {
            assert_eq!(at(&one, t), (Color { r: 1, g: 2, b: 3 }, 0.5));
        }
        assert_eq!(ramp_at(&[], 0.5), None);
    }

    /// Coincident stops make a hard edge whose sides are the list order.
    #[test]
    fn coincident_stops_make_a_hard_edge_in_list_order() {
        let stops = [
            stop(1, 0.0, 255, 0, 0, 1.0),
            stop(2, 0.5, 0, 255, 0, 1.0),
            stop(3, 0.5, 0, 0, 255, 1.0),
            stop(4, 1.0, 255, 255, 255, 1.0),
        ];
        assert!(at(&stops, 0.4999).0.g > 200, "left of the edge is green");
        assert_eq!(at(&stops, 0.5).0, Color { r: 0, g: 0, b: 255 });
        assert_eq!(at(&stops, 0.6).0.b, 255);
        // A stop moved past a neighbour sorts, and the list is untouched.
        let moved = [stop(1, 0.9, 255, 0, 0, 1.0), stop(2, 0.1, 0, 0, 0, 1.0)];
        let sorted = sorted_stops(&moved);
        assert_eq!(sorted[0].id, StopId::new(1, 2));
    }

    #[test]
    fn a_non_finite_position_reads_as_the_start() {
        let stops = [stop(1, 0.0, 5, 5, 5, 1.0), stop(2, 1.0, 9, 9, 9, 1.0)];
        assert_eq!(at(&stops, f64::NAN).0, Color { r: 5, g: 5, b: 5 });
    }
}
