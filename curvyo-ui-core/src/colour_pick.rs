//! The colour the eyedropper takes from the drawing
//! (`specs/0017-style-panel-rework` criteria 24 and 25, `adrs.md` decision 3;
//! `specs/0040-document-background` criteria 28 to 34): the stored colour, with
//! its alpha, of the topmost object whose painted stroke or painted fill is under
//! the point, else the document background where the point is on the document.

use curvyo_document_core::{
    BackgroundPaint, Color, DocumentBackground, DocumentSize, ObjectSnapshot, Opacity, Point,
    Tolerance,
};

use crate::hit_test_object::{distance_to_object, fills_point};

/// Which paint of an object a colour came from, or goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintTarget {
    /// The stroke.
    Stroke,
    /// The fill.
    Fill,
    /// The document background: the source of a pick over the empty document,
    /// and the target of the Background block's eyedropper.
    Background,
}

impl PaintTarget {
    /// The target named by the host (`"stroke"`, `"fill"`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "stroke" => Some(Self::Stroke),
            "fill" => Some(Self::Fill),
            "background" => Some(Self::Background),
            _ => None,
        }
    }

    /// The word that starts the status sentence after a pick ("Background color set to ...").
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Stroke => "Stroke",
            Self::Fill => "Fill",
            Self::Background => "Background",
        }
    }

    /// The name the host reads.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stroke => "stroke",
            Self::Fill => "fill",
            Self::Background => "background",
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
///
/// Where no object is painted, a point on the document (`size`, edges included)
/// takes the stored colour of a Solid `background`, alpha included; a None
/// background and the pasteboard take nothing.
#[must_use]
pub fn pick_colour(
    objects: &[ObjectSnapshot],
    point: Point,
    tolerance: Tolerance,
    size: DocumentSize,
    background: DocumentBackground,
) -> Option<PickedColour> {
    let from_objects = objects.iter().rev().find_map(|object| {
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
    });
    from_objects.or_else(|| {
        (background.paint == BackgroundPaint::Solid && size.contains(point)).then_some(
            PickedColour {
                color: background.color,
                opacity: background.opacity,
                paint: PaintTarget::Background,
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{Document, Length, NodeId, RectBounds, StyleEdit};

    use super::*;

    const TOL: f64 = 1.0;
    const PAGE: DocumentSize = DocumentSize::from_mm(100.0, 50.0);
    /// A None background: the tests of the objects see only the objects.
    const NO_BACKGROUND: DocumentBackground = DocumentBackground {
        paint: BackgroundPaint::None,
        ..DocumentBackground::DEFAULT
    };

    fn pick(objects: &[ObjectSnapshot], point: Point) -> Option<PickedColour> {
        pick_colour(objects, point, tol(), PAGE, NO_BACKGROUND)
    }

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
        let on_edge = pick(&all, Point::new(0.2, 10.0)).unwrap();
        assert_eq!((on_edge.color, on_edge.paint), (red(), PaintTarget::Stroke));
        let inside = pick(&all, Point::new(10.0, 10.0)).unwrap();
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
        assert_eq!(pick(&all, Point::new(10.0, 10.0)), None);
        assert_eq!(pick(&all, Point::new(100.0, 100.0)), None);
        assert_eq!(pick(&[], Point::new(0.0, 0.0)), None);
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
        assert_eq!(pick(&all, Point::new(0.2, 10.0)), None);
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
            pick(&all, Point::new(15.0, 5.0)).map(|p| p.color),
            Some(red()),
            "inside the upper one"
        );
        assert_eq!(
            pick(&all, Point::new(30.0, 30.0)).map(|p| p.color),
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
        let picked = pick(&objects(&document), Point::new(10.0, 10.0)).unwrap();
        assert_eq!(picked.opacity.get(), 0.0);
    }

    #[test]
    fn the_tolerance_widens_the_stroke_the_point_must_hit() {
        let document = Document::new(1);
        let id = square(&document, 0.0, 20.0);
        style(&document, id, &[StyleEdit::StrokeColor(red())]);
        let all = objects(&document);
        assert!(pick(&all, Point::new(-0.5, 10.0)).is_some());
        assert!(pick(&all, Point::new(-3.0, 10.0)).is_none());
    }

    fn background(r: u8, alpha: f64) -> DocumentBackground {
        DocumentBackground {
            paint: BackgroundPaint::Solid,
            color: Color { r, g: 0, b: 0 },
            opacity: Opacity::new(alpha).unwrap(),
        }
    }

    fn pick_on(
        objects: &[ObjectSnapshot],
        point: Point,
        background: DocumentBackground,
    ) -> Option<PickedColour> {
        pick_colour(objects, point, tol(), PAGE, background)
    }

    /// Criterion 29: empty document area takes the stored colour, alpha included.
    #[test]
    fn the_empty_document_gives_the_stored_background_colour() {
        let picked = pick_on(&[], Point::new(50.0, 25.0), background(255, 128.0 / 255.0)).unwrap();
        assert_eq!(picked.paint, PaintTarget::Background);
        assert_eq!(picked.color, red());
        assert_eq!(
            picked.opacity.get(),
            128.0 / 255.0,
            "stored, not composited"
        );
    }

    /// Criterion 28: objects keep priority, also one that lies partly on the pasteboard.
    #[test]
    fn an_object_wins_over_the_background() {
        let document = Document::new(1);
        let id = square(&document, 90.0, 40.0);
        style(
            &document,
            id,
            &[StyleEdit::FillEnabled(true), StyleEdit::FillColor(blue())],
        );
        let all = objects(&document);
        let on_page = pick_on(&all, Point::new(95.0, 10.0), background(255, 1.0)).unwrap();
        assert_eq!((on_page.paint, on_page.color), (PaintTarget::Fill, blue()));
        let on_pasteboard = pick_on(&all, Point::new(120.0, 10.0), background(255, 1.0)).unwrap();
        assert_eq!(on_pasteboard.paint, PaintTarget::Fill);
    }

    /// Criterion 31: an unfilled interior takes the background, the stroke its colour,
    /// and a fill with opacity 0 is a paint.
    #[test]
    fn a_none_fill_lets_the_background_through_but_a_transparent_fill_does_not() {
        let document = Document::new(1);
        let id = square(&document, 10.0, 30.0);
        let all = objects(&document);
        let inside = pick_on(&all, Point::new(25.0, 15.0), background(255, 1.0)).unwrap();
        assert_eq!(inside.paint, PaintTarget::Background);
        let on_stroke = pick_on(&all, Point::new(10.2, 15.0), background(255, 1.0)).unwrap();
        assert_eq!(on_stroke.paint, PaintTarget::Stroke);
        style(
            &document,
            id,
            &[
                StyleEdit::FillEnabled(true),
                StyleEdit::FillColor(blue()),
                StyleEdit::FillOpacity(Opacity::new(0.0).unwrap()),
            ],
        );
        let transparent = pick_on(
            &objects(&document),
            Point::new(25.0, 15.0),
            background(255, 1.0),
        )
        .unwrap();
        assert_eq!(
            (transparent.paint, transparent.color),
            (PaintTarget::Fill, blue())
        );
        assert_eq!(transparent.opacity.get(), 0.0);
    }

    /// Criteria 32 to 34: a None background and the pasteboard pick nothing; the
    /// document rectangle is closed, with no tolerance.
    #[test]
    fn none_and_the_pasteboard_pick_nothing_and_the_edge_is_the_document() {
        assert_eq!(pick_on(&[], Point::new(50.0, 25.0), NO_BACKGROUND), None);
        let solid = background(255, 1.0);
        for outside in [
            (-0.001, 10.0),
            (100.001, 10.0),
            (10.0, -0.001),
            (10.0, 50.001),
        ] {
            assert_eq!(
                pick_on(&[], Point::new(outside.0, outside.1), solid),
                None,
                "{outside:?}"
            );
        }
        for edge in [(0.0, 0.0), (100.0, 50.0), (0.0, 50.0)] {
            assert!(
                pick_on(&[], Point::new(edge.0, edge.1), solid).is_some(),
                "{edge:?}"
            );
        }
    }

    #[test]
    fn target_names_round_trip() {
        for target in [
            PaintTarget::Stroke,
            PaintTarget::Fill,
            PaintTarget::Background,
        ] {
            assert_eq!(PaintTarget::from_name(target.name()), Some(target));
        }
        assert_eq!(PaintTarget::from_name("both"), None);
    }
}
