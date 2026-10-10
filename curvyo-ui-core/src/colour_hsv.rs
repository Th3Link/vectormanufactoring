//! Hue, saturation and value of a colour and back, for the inline colour
//! picker (`specs/0017-style-panel-rework` criteria 17 to 21). The picker
//! sends h, s and v; this rounds them to the stored 8-bit RGB.

use curvyo_document_core::Color;

/// A colour as the picker holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    /// Degrees in `[0, 360)`; `None` for a grey or black, which has no hue
    /// (the picker keeps the hue it had, so dragging through a grey does not
    /// make the thumb jump, criterion 20).
    pub hue: Option<f64>,
    /// `[0, 1]`.
    pub saturation: f64,
    /// `[0, 1]`.
    pub value: f64,
}

/// The hue, saturation and value of `color`.
#[must_use]
pub fn rgb_to_hsv(color: Color) -> Hsv {
    let red = f64::from(color.r) / 255.0;
    let green = f64::from(color.g) / 255.0;
    let blue = f64::from(color.b) / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let value = max;
    let saturation = if max > 0.0 { delta / max } else { 0.0 };
    if delta <= 0.0 {
        return Hsv {
            hue: None,
            saturation,
            value,
        };
    }
    let sector = if (max - red).abs() < f64::EPSILON {
        ((green - blue) / delta).rem_euclid(6.0)
    } else if (max - green).abs() < f64::EPSILON {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    };
    Hsv {
        hue: Some((sector * 60.0).rem_euclid(360.0)),
        saturation,
        value,
    }
}

/// The colour of hue `hue` (degrees, any value; it wraps), saturation and
/// value (clamped to `[0, 1]`), rounded to the nearest 8-bit channel.
#[must_use]
pub fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> Color {
    let saturation = if saturation.is_finite() {
        saturation.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let value = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sector = if hue.is_finite() {
        hue.rem_euclid(360.0)
    } else {
        0.0
    } / 60.0;
    let chroma = value * saturation;
    let second = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match sector {
        n if n < 1.0 => (chroma, second, 0.0),
        n if n < 2.0 => (second, chroma, 0.0),
        n if n < 3.0 => (0.0, chroma, second),
        n if n < 4.0 => (0.0, second, chroma),
        n if n < 5.0 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let floor = value - chroma;
    let byte = |channel: f64| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let byte = ((channel + floor) * 255.0).round() as u8;
        byte
    };
    Color {
        r: byte(red),
        g: byte(green),
        b: byte(blue),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b }
    }

    #[test]
    fn the_primaries_and_secondaries_have_their_hues() {
        let cases = [
            (rgb(255, 0, 0), 0.0),
            (rgb(255, 255, 0), 60.0),
            (rgb(0, 255, 0), 120.0),
            (rgb(0, 255, 255), 180.0),
            (rgb(0, 0, 255), 240.0),
            (rgb(255, 0, 255), 300.0),
        ];
        for (color, hue) in cases {
            let hsv = rgb_to_hsv(color);
            assert_eq!(hsv.hue, Some(hue), "{color:?}");
            assert_eq!((hsv.saturation, hsv.value), (1.0, 1.0));
            assert_eq!(hsv_to_rgb(hue, 1.0, 1.0), color);
        }
    }

    #[test]
    fn a_grey_or_black_has_no_hue() {
        for level in [0, 1, 128, 255] {
            let hsv = rgb_to_hsv(rgb(level, level, level));
            assert_eq!(hsv.hue, None);
            assert_eq!(hsv.saturation, 0.0);
        }
        assert_eq!(rgb_to_hsv(rgb(0, 0, 0)).value, 0.0);
        assert_eq!(rgb_to_hsv(rgb(255, 255, 255)).value, 1.0);
    }

    #[test]
    fn any_hue_at_zero_value_or_saturation_gives_black_or_grey() {
        for hue in [0.0, 90.0, 215.0, 359.0] {
            assert_eq!(hsv_to_rgb(hue, 0.7, 0.0), rgb(0, 0, 0));
            assert_eq!(hsv_to_rgb(hue, 0.0, 0.5), rgb(128, 128, 128));
        }
    }

    #[test]
    fn rgb_to_hsv_to_rgb_is_exact_for_every_colour_on_a_grid() {
        for r in (0..=255).step_by(5) {
            for g in (0..=255).step_by(5) {
                for b in (0..=255).step_by(5) {
                    let color = rgb(r, g, b);
                    let hsv = rgb_to_hsv(color);
                    let back = hsv_to_rgb(hsv.hue.unwrap_or(0.0), hsv.saturation, hsv.value);
                    assert_eq!(back, color, "{color:?} via {hsv:?}");
                }
            }
        }
    }

    #[test]
    fn hue_wraps_and_the_inputs_are_clamped() {
        assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), hsv_to_rgb(0.0, 1.0, 1.0));
        assert_eq!(hsv_to_rgb(-60.0, 1.0, 1.0), hsv_to_rgb(300.0, 1.0, 1.0));
        assert_eq!(hsv_to_rgb(0.0, 2.0, 5.0), rgb(255, 0, 0));
        assert_eq!(hsv_to_rgb(f64::NAN, f64::NAN, f64::INFINITY), rgb(0, 0, 0));
    }
}
