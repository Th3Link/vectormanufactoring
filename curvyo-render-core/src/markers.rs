//! The shapes of the markers and their triangles
//! (`specs/0018-stroke-markers` criteria 4 and 5, `adrs.md` decisions 2, 3
//! and 5): a filled arrow, an isosceles triangle 4 w long and 3 w wide, or a
//! filled dot 3 w across, centred on its anchor, in the stroke's colour. They
//! are appended to the stroke's own layer, so a pixel where a marker and its
//! stroke overlap is written once and a translucent stroke does not darken.

use curvyo_document_core::{MarkerShape, Point, Vec2};
use lyon::math::point;
use lyon::path::Path;

use crate::color::RgbaColor;
use crate::fill;
use crate::glyphs::{DrawList, Vertex};
use crate::marker_place::MarkerPlacement;

/// The most markers one frame draws across all objects; past it further
/// markers are skipped, never the stroke.
pub(crate) const MAX_MARKERS_PER_FRAME: usize = 50_000;

/// The markers left for the rest of one frame.
#[derive(Debug)]
pub(crate) struct MarkerBudget {
    remaining: usize,
}

impl MarkerBudget {
    pub(crate) const fn per_frame() -> Self {
        Self {
            remaining: MAX_MARKERS_PER_FRAME,
        }
    }

    /// Takes up to `wanted` markers from the budget, returns how many.
    pub(crate) fn take(&mut self, wanted: usize) -> usize {
        let granted = wanted.min(self.remaining);
        self.remaining -= granted;
        granted
    }
}

/// The arrow is this many stroke widths long and this many wide.
const ARROW_LENGTH: f64 = 4.0;
const ARROW_WIDTH: f64 = 3.0;
/// The dot is this many stroke widths across.
const DOT_DIAMETER: f64 = 3.0;

fn vertex(position: Point, color: RgbaColor) -> Vertex {
    Vertex { position, color }
}

/// The triangles of `placements`, each shape sized from `width_mm` (the drawn
/// stroke width) and painted in `color`.
pub(crate) fn marker_triangles(
    placements: &[MarkerPlacement],
    width_mm: f64,
    color: RgbaColor,
    tolerance_mm: f64,
) -> Vec<Vertex> {
    let mut triangles = Vec::new();
    for placement in placements {
        match placement.shape {
            MarkerShape::None => {}
            MarkerShape::Arrow => {
                let along = placement.direction;
                let across = Vec2::new(-along.y, along.x);
                let half_length = ARROW_LENGTH * width_mm / 2.0;
                let half_width = ARROW_WIDTH * width_mm / 2.0;
                let centre = placement.point;
                let tip = centre.translated(along.scaled(half_length));
                let base = centre.translated(along.scaled(-half_length));
                triangles.push(vertex(tip, color));
                triangles.push(vertex(base.translated(across.scaled(half_width)), color));
                triangles.push(vertex(base.translated(across.scaled(-half_width)), color));
            }
            MarkerShape::Dot => {
                let mut builder = Path::builder();
                #[allow(clippy::cast_possible_truncation)]
                builder.add_circle(
                    point(placement.point.x as f32, placement.point.y as f32),
                    (DOT_DIAMETER * width_mm / 2.0) as f32,
                    lyon::path::Winding::Positive,
                );
                let dot: DrawList = fill::fill(&builder.build(), color, tolerance_mm);
                triangles.extend(dot.triangles);
            }
        }
    }
    triangles
}
