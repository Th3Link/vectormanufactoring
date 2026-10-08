//! A gradient fill as the renderer hands it to the host
//! (`specs/0007-stroke-and-fill-styling` criteria 16, 21, 22, 35): the colour
//! ramp as texels, the box the gradient spans, and the vertex range it paints.
//! The host looks the ramp up per pixel; this module is everything about it
//! that can be tested natively.

use curvyo_document_core::{Angle, GradientStop, Point, ramp_at, sorted_stops};

use crate::glyphs::DrawList;

/// How many texels a ramp has. 256 steps per ramp are finer than 8-bit
/// channels can show along any realistic object.
pub const RAMP_TEXELS: usize = 256;

/// How many gradient fills a frame paints from a ramp; the host's ramp texture
/// has one row per gradient. Beyond this the fills stay flat in their first
/// colour. Far above any document an editor has to keep interactive.
pub const MAX_GRADIENTS: usize = 1024;

/// One gradient's colours from `t = 0` to `t = 1`, as non-premultiplied
/// sRGB-encoded RGBA8 texels: a host that filters linearly between them
/// interpolates the encoded values, which is SVG's default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ramp(pub [[u8; 4]; RAMP_TEXELS]);

impl Ramp {
    /// The ramp of `stops` (any order; sorted here); `None` without stops.
    #[must_use]
    pub fn from_stops(stops: &[GradientStop]) -> Option<Self> {
        let sorted = sorted_stops(stops);
        let mut texels = [[0_u8; 4]; RAMP_TEXELS];
        for (index, texel) in texels.iter_mut().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let t = index as f64 / (RAMP_TEXELS - 1) as f64;
            let (color, opacity) = ramp_at(&sorted, t)?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let alpha = (opacity.get() * 255.0).round() as u8;
            *texel = [color.r, color.g, color.b, alpha];
        }
        Some(Self(texels))
    }

    /// The colour at `t = 0`: what a host without a ramp lookup paints.
    #[must_use]
    pub const fn first(&self) -> [u8; 4] {
        self.0[0]
    }
}

/// The box a gradient spans: the object's oriented selection box
/// (`curvyo-ui-core::OrientedBox`: a local-frame rectangle, and the angle and
/// pivot that turn the local frame into document space). `curvyo-editor-wasm`
/// fills it in; this crate has no `ui-core` dependency, so it takes the plain
/// values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientFrame {
    /// Minimum corner, local frame.
    pub min: Point,
    /// Maximum corner, local frame.
    pub max: Point,
    /// How far the local frame is turned from the document axes.
    pub angle: Angle,
    /// The point the local frame turns about.
    pub pivot: Point,
}

/// A side of a degenerate box counts as this long, so a flat box never divides
/// by zero (its fill has no area to show anyway).
const MIN_EXTENT_MM: f64 = 1e-9;

impl GradientFrame {
    /// The linear gradient coordinate of `point`: `x` runs 0 at the box's left
    /// edge to 1 at its right edge in the object's own frame (angle 0, from
    /// (0, 0.5) to (1, 0.5)); `y` is unused.
    #[must_use]
    pub fn linear(&self, point: Point) -> [f32; 2] {
        let local = point.rotated_around(self.pivot, Angle::from_radians(-self.angle.as_radians()));
        let width = (self.max.x - self.min.x).max(MIN_EXTENT_MM);
        #[allow(clippy::cast_possible_truncation)]
        [((local.x - self.min.x) / width) as f32, 0.0]
    }

    /// The radial gradient coordinate of `point`: its offset from the box
    /// centre in units of half the box, so the ramp's `t` is the vector's
    /// length (centre 0, the ellipse inscribed in the box 1).
    #[must_use]
    pub fn radial(&self, point: Point) -> [f32; 2] {
        let local = point.rotated_around(self.pivot, Angle::from_radians(-self.angle.as_radians()));
        let half_w = ((self.max.x - self.min.x) / 2.0).max(MIN_EXTENT_MM);
        let half_h = ((self.max.y - self.min.y) / 2.0).max(MIN_EXTENT_MM);
        let cx = f64::midpoint(self.min.x, self.max.x);
        let cy = f64::midpoint(self.min.y, self.max.y);
        #[allow(clippy::cast_possible_truncation)]
        [
            ((local.x - cx) / half_w) as f32,
            ((local.y - cy) / half_h) as f32,
        ]
    }
}

/// One gradient fill in a [`crate::DrawList`]: the vertices `start..end` of its
/// triangle list are the fill's tessellation (flat-coloured with the ramp's
/// first colour, which is what a host that cannot sample the ramp shows), to
/// be painted from `ramp` by the coordinate of each vertex.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientFill {
    /// First vertex of the fill's triangles.
    pub start: usize,
    /// One past the last vertex.
    pub end: usize,
    /// The box the gradient spans.
    pub frame: GradientFrame,
    /// Radial (the coordinate's length is `t`) rather than linear (its `x`).
    pub radial: bool,
    /// The colours.
    pub ramp: Ramp,
}

impl GradientFill {
    /// The gradient coordinate of a vertex at `point`: see
    /// [`GradientFrame::linear`] and [`GradientFrame::radial`].
    #[must_use]
    pub fn coordinate(&self, point: Point) -> [f32; 2] {
        if self.radial {
            self.frame.radial(point)
        } else {
            self.frame.linear(point)
        }
    }
}

impl DrawList {
    /// The gradient fills among the artwork: each names the vertices of its
    /// fill and carries its ramp. Those vertices hold the ramp's first colour
    /// as their flat colour.
    #[must_use]
    pub fn gradients(&self) -> &[GradientFill] {
        &self.gradients
    }

    /// Records a gradient fill. Only the artwork builder calls this, right
    /// after it appended the fill's vertices.
    pub(crate) fn push_gradient(&mut self, fill: GradientFill) {
        self.gradients.push(fill);
    }

    /// The gradient fills a host paints from a ramp texture: the first
    /// [`MAX_GRADIENTS`] of them. Later ones stay flat in their first colour.
    #[must_use]
    pub fn painted_gradients(&self) -> &[GradientFill] {
        self.gradients
            .get(..MAX_GRADIENTS)
            .unwrap_or(&self.gradients)
    }

    /// Per-vertex gradient attributes for a host with a ramp texture of `rows`
    /// rows (one per painted gradient, in order): `[x, y, v, mode]`. `x` and
    /// `y` are the vertex's [`GradientFill::coordinate`]; `v` is the texture
    /// coordinate of the ramp's row (its centre, so linear filtering never mixes
    /// two ramps); `mode` is 0 for a flat vertex, 1 for a linear and 2 for a
    /// radial gradient. The host takes `t` as `x` (linear) or `length(x, y)`
    /// (radial) and clamps it to 0 to 1 (SVG's "pad").
    #[must_use]
    pub fn gradient_attributes(&self, rows: usize) -> Vec<[f32; 4]> {
        let mut attributes = vec![[0.0_f32; 4]; self.triangles.len()];
        let rows = rows.max(1);
        for (index, fill) in self.painted_gradients().iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let v = (index as f32 + 0.5) / rows as f32;
            let mode = if fill.radial { 2.0 } else { 1.0 };
            let (Some(vertices), Some(slots)) = (
                self.triangles.get(fill.start..fill.end),
                attributes.get_mut(fill.start..fill.end),
            ) else {
                continue;
            };
            for (vertex, attribute) in vertices.iter().zip(slots) {
                let [x, y] = fill.coordinate(vertex.position);
                *attribute = [x, y, v, mode];
            }
        }
        attributes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{Color, Opacity, StopId, StopPosition};

    fn stop(n: u64, at: f64, color: Color) -> GradientStop {
        GradientStop {
            id: StopId::new(1, n),
            position: StopPosition::new(at).unwrap(),
            color,
            opacity: Opacity::OPAQUE,
        }
    }

    const RED: Color = Color { r: 255, g: 0, b: 0 };
    const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };

    /// The golden ramp of red to white: the end texels, the middle and a
    /// quarter point are pinned byte for byte.
    #[test]
    fn the_default_ramp_is_pinned_texel_by_texel() {
        let ramp = Ramp::from_stops(&[stop(1, 0.0, RED), stop(2, 1.0, WHITE)]).unwrap();
        assert_eq!(ramp.0[0], [255, 0, 0, 255]);
        assert_eq!(ramp.0[255], [255, 255, 255, 255]);
        assert_eq!(ramp.0[128], [255, 128, 128, 255]);
        assert_eq!(ramp.0[64], [255, 64, 64, 255]);
        for texel in &ramp.0 {
            assert_eq!(texel[0], 255);
            assert_eq!(texel[1], texel[2]);
        }
        assert!(ramp.0.windows(2).all(|pair| pair[0][1] <= pair[1][1]));
    }

    #[test]
    fn a_ramp_needs_a_stop_and_one_stop_is_uniform() {
        assert_eq!(Ramp::from_stops(&[]), None);
        let one = Ramp::from_stops(&[stop(1, 0.3, RED)]).unwrap();
        assert!(one.0.iter().all(|texel| *texel == [255, 0, 0, 255]));
    }

    /// Coincident stops are a hard edge in the texels.
    #[test]
    fn coincident_stops_cut_the_ramp() {
        let blue = Color { r: 0, g: 0, b: 255 };
        let ramp = Ramp::from_stops(&[
            stop(1, 0.0, RED),
            stop(2, 0.5, RED),
            stop(3, 0.5, blue),
            stop(4, 1.0, blue),
        ])
        .unwrap();
        assert_eq!(ramp.0[100], [255, 0, 0, 255]);
        assert_eq!(ramp.0[128], [0, 0, 255, 255]);
    }

    fn frame(angle_deg: f64) -> GradientFrame {
        GradientFrame {
            min: Point::new(0.0, 0.0),
            max: Point::new(20.0, 10.0),
            angle: Angle::from_radians(angle_deg.to_radians()),
            pivot: Point::new(10.0, 5.0),
        }
    }

    fn near(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
    }

    #[test]
    fn a_linear_gradient_runs_left_to_right_across_the_box() {
        let f = frame(0.0);
        assert!(near(f.linear(Point::new(0.0, 3.0)), [0.0, 0.0]));
        assert!(near(f.linear(Point::new(5.0, 3.0)), [0.25, 0.0]));
        assert!(near(f.linear(Point::new(20.0, 9.0)), [1.0, 0.0]));
        // Outside the box the coordinate runs on; the ramp pads.
        assert!(f.linear(Point::new(-4.0, 0.0))[0] < 0.0);
    }

    /// Rotated 90 degrees the ramp runs top to bottom on screen.
    #[test]
    fn a_rotated_box_turns_the_gradient_with_it() {
        let f = frame(90.0);
        // The box's local +x axis now points down (y grows with local x): the
        // box centre is the pivot; local (0, 5) maps to (10, -5), local (20, 5)
        // to (10, 15).
        assert!(near(f.linear(Point::new(10.0, -5.0)), [0.0, 0.0]));
        assert!(near(f.linear(Point::new(10.0, 15.0)), [1.0, 0.0]));
        assert!(near(f.linear(Point::new(10.0, 5.0)), [0.5, 0.0]));
    }

    #[test]
    fn a_radial_gradient_is_the_ellipse_inscribed_in_the_box() {
        let f = frame(0.0);
        let at = |x, y| {
            let c = f.radial(Point::new(x, y));
            c[0].hypot(c[1])
        };
        assert!(at(10.0, 5.0).abs() < 1e-6, "centre");
        assert!((at(20.0, 5.0) - 1.0).abs() < 1e-6, "right edge");
        assert!((at(10.0, 0.0) - 1.0).abs() < 1e-6, "top edge");
        assert!((at(15.0, 5.0) - 0.5).abs() < 1e-6, "elliptical, not round");
    }

    #[test]
    fn a_zero_size_box_gives_finite_coordinates() {
        let flat = GradientFrame {
            min: Point::new(3.0, 3.0),
            max: Point::new(3.0, 3.0),
            angle: Angle::from_radians(0.0),
            pivot: Point::new(3.0, 3.0),
        };
        for c in [
            flat.linear(Point::new(4.0, 4.0)),
            flat.radial(Point::new(4.0, 4.0)),
        ] {
            assert!(c[0].is_finite() && c[1].is_finite());
        }
    }
}
