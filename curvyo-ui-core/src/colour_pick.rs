//! The colour the eyedropper takes from the drawing
//! (`specs/0017-style-panel-rework` criteria 24 and 25, `adrs.md` decision 3):
//! the stored colour, with its alpha, of the topmost object whose painted
//! stroke or painted fill is under the point.

use curvyo_document_core::{Color, ObjectSnapshot, Opacity, Point, Tolerance};

use crate::hit_test_object::{distance_to_object, fills_point};

/// Which paint of an object a colour came from, or goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintTarget {
    /// The stroke.
    Stroke,
    /// The fill.
    Fill,
}

impl PaintTarget {
    /// The target named by the host (`"stroke"`, `"fill"`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "stroke" => Some(Self::Stroke),
            "fill" => Some(Self::Fill),
            _ => None,
        }
    }

    /// The name the host reads.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stroke => "stroke",
            Self::Fill => "fill",
        }
    }
}

/// A colour picked from the drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickedColour {
    /// The red, green and blue as stored.
    pub color: Color,
    /// The alpha as stored.
    pub opacity: Opacity,
    /// The paint it came from.
    pub paint: PaintTarget,
}

/// The colour under `point`, or `None` where nothing is painted. `objects` is
/// in z-order, bottom to top. Topmost first: the first object whose stroke
/// paints (`enabled`, any opacity) and whose outline is within `tolerance` of
/// the point gives its stroke colour; else, if its fill paints and contains
/// the point, its fill colour; else the next object down. An object whose
/// paint is None is not pickable by that paint. Unlike the Select tool's rule
/// this is topmost-first, not nearest-outline.
#[must_use]
pub fn pick_colour(
    objects: &[ObjectSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> Option<PickedColour> {
    objects.iter().rev().find_map(|object| {
        let style = object.style();
        if style.stroke.enabled && distance_to_object(object, point, tolerance).is_some() {
            return Some(PickedColour {
                color: style.stroke.color,
                opacity: style.stroke.opacity,
                paint: PaintTarget::Stroke,
            });
        }
        fills_point(object, point).then_some(PickedColour {
            color: style.fill.color,
            opacity: style.fill.opacity,
            paint: PaintTarget::Fill,
        })
    })
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{Document, Length, NodeId, RectBounds, StyleEdit};

    use super::*;

    const TOL: f64 = 1.0;

    fn tol() -> Tolerance {
        Tolerance::from_mm(TOL)
    }

    fn square(document: &Document, x: f64, size: f64) -> NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(x, 0.0),
            width: Length::from_mm(size),
            height: Length::from_mm(size),
        })
    }

    fn objects(document: &Document) -> Vec<ObjectSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect()
    }

    fn red() -> Color {
        Color { r: 255, g: 0, b: 0 }
    }

    fn blue() -> Color {
        Color { r: 0, g: 0, b: 255 }
    }

    fn style(document: &Document, id: NodeId, edits: &[StyleEdit]) {
        for edit in edits {
            document.edit_style(&[id], edit).unwrap();
        }
    }

    #[test]
    fn a_point_on_the_stroke_picks_the_stroke_and_inside_picks_the_fill() {
        let document = Document::new(1);
        let id = square(&document, 0.0, 20.0);
        style(
            &document,
            id,
            &[
                StyleEdit::StrokeColor(red()),
                StyleEdit::FillEnabled(true),
                StyleEdit::FillColor(blue()),
                StyleEdit::FillOpacity(Opacity::new(0.5).unwrap()),
            ],
        );
        let all = objects(&document);
        let on_edge = pick_colour(&all, Point::new(0.2, 10.0), tol()).unwrap();
        assert_eq!((on_edge.color, on_edge.paint), (red(), PaintTarget::Stroke));
        let inside = pick_colour(&all, Point::new(10.0, 10.0), tol()).unwrap();
        assert_eq!(inside.color, blue());
        assert_eq!(inside.opacity, Opacity::new(0.5).unwrap(), "with its alpha");
        assert_eq!(inside.paint, PaintTarget::Fill);
    }

    #[test]
    fn nothing_painted_gives_none() {
        let document = Document::new(1);
        let _ = square(&document, 0.0, 20.0);
        let all = objects(&document);
        // Inside an unfilled rectangle and far outside: no paint there.
        assert_eq!(pick_colour(&all, Point::new(10.0, 10.0), tol()), None);
        assert_eq!(pick_colour(&all, Point::new(100.0, 100.0), tol()), None);
        assert_eq!(pick_colour(&[], Point::new(0.0, 0.0), tol()), None);
    }

    #[test]
    fn a_paint_of_none_is_not_pickable_by_that_paint() {
        let document = Document::new(1);
        let id = square(&document, 0.0, 20.0);
        style(
            &document,
            id,
            &[StyleEdit::StrokeEnabled(false), StyleEdit::FillColor(red())],
        );
        let all = objects(&document);
        assert_eq!(pick_colour(&all, Point::new(0.2, 10.0), tol()), None);
    }

    #[test]
    fn the_topmost_painted_object_wins_and_a_hole_lets_the_one_below_through() {
        let document = Document::new(1);
        let below = square(&document, 0.0, 40.0);
        let above = square(&document, 10.0, 10.0);
        style(
            &document,
            below,
            &[StyleEdit::FillEnabled(true), StyleEdit::FillColor(blue())],
        );
        style(
            &document,
            above,
            &[StyleEdit::FillEnabled(true), StyleEdit::FillColor(red())],
        );
        let all = objects(&document);
        assert_eq!(
            pick_colour(&all, Point::new(15.0, 5.0), tol()).map(|p| p.color),
            Some(red()),
            "inside the upper one"
        );
        assert_eq!(
            pick_colour(&all, Point::new(30.0, 30.0), tol()).map(|p| p.color),
            Some(blue()),
            "outside the upper one, the lower one shows"
        );
    }

    #[test]
    fn a_paint_with_opacity_zero_still_counts() {
        let document = Document::new(1);
        let id = square(&document, 0.0, 20.0);
        style(
            &document,
            id,
            &[
                StyleEdit::FillEnabled(true),
                StyleEdit::FillColor(red()),
                StyleEdit::FillOpacity(Opacity::new(0.0).unwrap()),
            ],
        );
        let picked = pick_colour(&objects(&document), Point::new(10.0, 10.0), tol()).unwrap();
        assert_eq!(picked.opacity.get(), 0.0);
    }

    #[test]
    fn the_tolerance_widens_the_stroke_the_point_must_hit() {
        let document = Document::new(1);
        let id = square(&document, 0.0, 20.0);
        style(&document, id, &[StyleEdit::StrokeColor(red())]);
        let all = objects(&document);
        assert!(pick_colour(&all, Point::new(-0.5, 10.0), tol()).is_some());
        assert!(pick_colour(&all, Point::new(-3.0, 10.0), tol()).is_none());
    }

    #[test]
    fn target_names_round_trip() {
        for target in [PaintTarget::Stroke, PaintTarget::Fill] {
            assert_eq!(PaintTarget::from_name(target.name()), Some(target));
        }
        assert_eq!(PaintTarget::from_name("both"), None);
    }
}
