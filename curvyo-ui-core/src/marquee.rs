//! The marquee's box arithmetic (`specs/advanced-selection/specification.md`,
//! acceptance criteria 9 to 11): which mode a drag has and which objects a
//! rectangle selects in each mode. Pure functions of points and snapshots;
//! the gesture state is in `select_tool`.

use curvyo_document_core::{NodeId, ObjectSnapshot, Point, Tolerance};

use crate::oriented_box::oriented_bounds;

/// What a marquee selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarqueeMode {
    /// Every object whose box the rectangle fully contains or crosses
    /// (green, a leftward drag by default).
    Touch,
    /// Only objects whose box lies entirely inside the rectangle (red, a
    /// rightward drag by default).
    Contain,
}

impl MarqueeMode {
    /// The mode of a drag from `start` to `current` (criteria 9 to 11): a
    /// drag whose end has a greater x than its start is [`Self::Contain`];
    /// leftward, and a drag with no net horizontal movement, is
    /// [`Self::Touch`]. `alt` inverts the result. Document x and screen x
    /// have the same sign (the view is a scale plus an origin), so the test
    /// runs on document points.
    #[must_use]
    pub fn for_drag(start: Point, current: Point, alt: bool) -> Self {
        let rightward = current.x > start.x;
        if rightward ^ alt {
            Self::Contain
        } else {
            Self::Touch
        }
    }
}

/// The objects `mode` selects for the rectangle with opposite corners `from`
/// and `to`, in the order of `objects` (z-order). Each object is measured by
/// the box the Select tool draws around it ([`oriented_bounds`]): Contain
/// needs all four corners of that box inside the rectangle, Touch needs the
/// axis-aligned box around those corners to overlap the rectangle. Edges are
/// inclusive within `tolerance`.
#[must_use]
pub fn objects_in_marquee(
    objects: &[ObjectSnapshot],
    from: Point,
    to: Point,
    mode: MarqueeMode,
    tolerance: Tolerance,
) -> Vec<NodeId> {
    let slack = tolerance.as_mm();
    let (low_x, high_x) = (from.x.min(to.x) - slack, from.x.max(to.x) + slack);
    let (low_y, high_y) = (from.y.min(to.y) - slack, from.y.max(to.y) + slack);
    objects
        .iter()
        .filter(|object| {
            let corners = oriented_bounds(object).document_corners();
            let (min_x, max_x) = extent(corners.iter().map(|c| c.x));
            let (min_y, max_y) = extent(corners.iter().map(|c| c.y));
            match mode {
                MarqueeMode::Contain => {
                    min_x >= low_x && max_x <= high_x && min_y >= low_y && max_y <= high_y
                }
                MarqueeMode::Touch => {
                    max_x >= low_x && min_x <= high_x && max_y >= low_y && min_y <= high_y
                }
            }
        })
        .map(ObjectSnapshot::id)
        .collect()
}

/// The smallest and largest of `values`.
fn extent(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), v| {
        (low.min(v), high.max(v))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_4;

    use curvyo_document_core::{Angle, Document, Length, RectBounds};

    const TOLERANCE: Tolerance = Tolerance::from_mm(1e-6);

    fn rect(document: &Document, x: f64, y: f64, w: f64, h: f64) -> ObjectSnapshot {
        let id = document.create_rect(RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        });
        document.object(id).expect("exists")
    }

    /// AC 9 / 10: rightward is contain, leftward and "no net horizontal
    /// movement" are touch.
    #[test]
    fn direction_picks_the_mode_and_a_vertical_drag_counts_as_leftward() {
        let start = Point::new(10.0, 10.0);
        assert_eq!(
            MarqueeMode::for_drag(start, Point::new(20.0, 0.0), false),
            MarqueeMode::Contain
        );
        assert_eq!(
            MarqueeMode::for_drag(start, Point::new(0.0, 30.0), false),
            MarqueeMode::Touch
        );
        assert_eq!(
            MarqueeMode::for_drag(start, Point::new(10.0, 50.0), false),
            MarqueeMode::Touch
        );
    }

    /// AC 11: Alt inverts the mode in both directions.
    #[test]
    fn alt_inverts_the_mode() {
        let start = Point::new(10.0, 10.0);
        assert_eq!(
            MarqueeMode::for_drag(start, Point::new(20.0, 10.0), true),
            MarqueeMode::Touch
        );
        assert_eq!(
            MarqueeMode::for_drag(start, Point::new(0.0, 10.0), true),
            MarqueeMode::Contain
        );
        assert_eq!(
            MarqueeMode::for_drag(start, start, true),
            MarqueeMode::Contain,
            "no net horizontal movement is touch, inverted"
        );
    }

    #[test]
    fn contain_selects_only_objects_fully_inside() {
        let document = Document::new(1);
        let inside = rect(&document, 2.0, 2.0, 4.0, 4.0);
        let crossing = rect(&document, 8.0, 2.0, 6.0, 4.0);
        let outside = rect(&document, 30.0, 30.0, 4.0, 4.0);
        let objects = vec![inside.clone(), crossing, outside];
        let found = objects_in_marquee(
            &objects,
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            MarqueeMode::Contain,
            TOLERANCE,
        );
        assert_eq!(found, vec![inside.id()]);
    }

    #[test]
    fn touch_selects_inside_and_crossing_but_not_apart() {
        let document = Document::new(1);
        let inside = rect(&document, 2.0, 2.0, 4.0, 4.0);
        let crossing = rect(&document, 8.0, 2.0, 6.0, 4.0);
        let outside = rect(&document, 30.0, 30.0, 4.0, 4.0);
        let objects = vec![inside.clone(), crossing.clone(), outside];
        let found = objects_in_marquee(
            &objects,
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            MarqueeMode::Touch,
            TOLERANCE,
        );
        assert_eq!(found, vec![inside.id(), crossing.id()]);
    }

    /// A marquee that fully contains an object also touches it, and the
    /// result does not depend on which corner the drag started at.
    #[test]
    fn the_rectangle_is_the_same_whichever_corner_started_it() {
        let document = Document::new(1);
        let a = rect(&document, 2.0, 2.0, 4.0, 4.0);
        let objects = vec![a.clone()];
        for (from, to) in [
            (Point::new(0.0, 0.0), Point::new(10.0, 10.0)),
            (Point::new(10.0, 10.0), Point::new(0.0, 0.0)),
            (Point::new(10.0, 0.0), Point::new(0.0, 10.0)),
        ] {
            for mode in [MarqueeMode::Touch, MarqueeMode::Contain] {
                assert_eq!(
                    objects_in_marquee(&objects, from, to, mode, TOLERANCE),
                    vec![a.id()]
                );
            }
        }
    }

    /// An object whose box only touches the rectangle's edge is touched, and
    /// one that lies exactly on it is contained (edges are inclusive).
    #[test]
    fn edges_are_inclusive() {
        let document = Document::new(1);
        let on_edge = rect(&document, 0.0, 0.0, 10.0, 10.0);
        let beside = rect(&document, 10.0, 0.0, 5.0, 5.0);
        let objects = vec![on_edge.clone(), beside.clone()];
        let from = Point::new(0.0, 0.0);
        let to = Point::new(10.0, 10.0);
        assert_eq!(
            objects_in_marquee(&objects, from, to, MarqueeMode::Contain, TOLERANCE),
            vec![on_edge.id()]
        );
        assert_eq!(
            objects_in_marquee(&objects, from, to, MarqueeMode::Touch, TOLERANCE),
            vec![on_edge.id(), beside.id()]
        );
    }

    /// A rotated object is measured by the corners of the box the maker sees
    /// (the forward note of the specification): a square turned 45 degrees
    /// has a wider box than its frame.
    #[test]
    fn a_rotated_object_is_contained_only_when_its_turned_box_fits() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(-5.0, -5.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let square = document.object(id).expect("exists");
        let turned = square.rotated(Point::new(0.0, 0.0), Angle::from_radians(FRAC_PI_4));
        document.rotate_object(&turned).expect("rotates");
        let objects = vec![document.object(id).expect("exists")];
        // The turned box reaches about 7.07 mm from the centre.
        let tight = (Point::new(-6.0, -6.0), Point::new(6.0, 6.0));
        let wide = (Point::new(-8.0, -8.0), Point::new(8.0, 8.0));
        assert_eq!(
            objects_in_marquee(&objects, tight.0, tight.1, MarqueeMode::Contain, TOLERANCE),
            Vec::<NodeId>::new()
        );
        assert_eq!(
            objects_in_marquee(&objects, wide.0, wide.1, MarqueeMode::Contain, TOLERANCE),
            vec![id]
        );
    }
}
