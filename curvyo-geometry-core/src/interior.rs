//! Whether a point lies inside the area a path's fill paints
//! (`specs/0007-stroke-and-fill-styling/adrs.md`, section 6): a non-zero
//! winding test on the exact cubic outline, built on `kurbo`.

use curvyo_document_core::{Point, Vec2};
use kurbo::{BezPath, PathEl, Point as KurboPoint, Shape};

fn to_kurbo(point: Point) -> KurboPoint {
    KurboPoint::new(point.x, point.y)
}

/// One anchor as `(point, handle_in, handle_out)`, the handles relative to
/// the point: the triple `curvyo-ui-core` already reads every outline as.
pub type OutlineTriple = (Point, Vec2, Vec2);

fn cubic_to(path: &mut BezPath, from: OutlineTriple, to: OutlineTriple) {
    path.push(PathEl::CurveTo(
        to_kurbo(from.0.translated(from.2)),
        to_kurbo(to.0.translated(to.1)),
        to_kurbo(to.0),
    ));
}

/// Whether `query` lies in the interior of the outline through `anchors`,
/// by the non-zero winding rule, the same rule the fill is painted with.
///
/// A `closed` outline is bounded by its real closing segment, the cubic
/// through its own handles; an open one is closed with a straight chord from
/// its last anchor back to its first, as the fill is (acceptance criteria 15
/// and 23). Fewer than two anchors have no interior. The test is exact on
/// cubics, so it takes no tolerance: points near the edge are outline hits at
/// the caller's hit tolerance before this test runs.
#[must_use]
pub fn contains_point(anchors: &[OutlineTriple], closed: bool, query: Point) -> bool {
    let (Some(first), Some(last)) = (anchors.first(), anchors.last()) else {
        return false;
    };
    if anchors.len() < 2 {
        return false;
    }
    let mut path = BezPath::new();
    path.move_to(to_kurbo(first.0));
    for pair in anchors.windows(2) {
        cubic_to(&mut path, pair[0], pair[1]);
    }
    if closed {
        cubic_to(&mut path, *last, *first);
    }
    path.close_path();
    path.winding(to_kurbo(query)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corner(_id: u64, x: f64, y: f64) -> OutlineTriple {
        (Point::new(x, y), Vec2::ZERO, Vec2::ZERO)
    }

    fn square() -> Vec<OutlineTriple> {
        vec![
            corner(1, 0.0, 0.0),
            corner(2, 10.0, 0.0),
            corner(3, 10.0, 10.0),
            corner(4, 0.0, 10.0),
        ]
    }

    #[test]
    fn a_closed_square_contains_its_inside_and_not_its_outside() {
        let anchors = square();
        assert!(contains_point(&anchors, true, Point::new(5.0, 5.0)));
        assert!(!contains_point(&anchors, true, Point::new(11.0, 5.0)));
        assert!(!contains_point(&anchors, true, Point::new(-0.1, 5.0)));
    }

    /// Acceptance criterion 15 and 23: an open path's interior is the area
    /// its fill paints, closed with a straight chord from the last node to
    /// the first.
    #[test]
    fn an_open_u_shape_is_closed_with_a_chord() {
        // A U: down the left side, across the bottom, up the right. The chord
        // from the last node (top right) back to the first (top left) closes
        // the top.
        let anchors = vec![
            corner(1, 0.0, 0.0),
            corner(2, 0.0, 10.0),
            corner(3, 10.0, 10.0),
            corner(4, 10.0, 0.0),
        ];
        assert!(contains_point(&anchors, false, Point::new(5.0, 5.0)));
        assert!(!contains_point(&anchors, false, Point::new(5.0, -1.0)));
    }

    /// Criterion 23 (architect, section 2.6): a closed path's closing
    /// segment is the real cubic through its handles, not a chord.
    #[test]
    fn a_closed_path_is_bounded_by_its_curved_closing_segment() {
        // Anchor 3 at (10, 0) and anchor 1 at (0, 0), joined by a closing
        // cubic that bulges up to y = -7.5 at its middle.
        let mut anchors = vec![
            corner(1, 0.0, 0.0),
            corner(2, 5.0, 10.0),
            corner(3, 10.0, 0.0),
        ];
        anchors[2].2 = Vec2::new(0.0, -10.0);
        anchors[0].1 = Vec2::new(0.0, -10.0);
        let above_chord = Point::new(5.0, -3.0);
        assert!(contains_point(&anchors, true, above_chord));
        assert!(
            !contains_point(&anchors, false, above_chord),
            "open: the chord closes it, nothing lies above the chord"
        );
    }

    #[test]
    fn fewer_than_two_anchors_have_no_interior() {
        assert!(!contains_point(&[], true, Point::new(0.0, 0.0)));
        assert!(!contains_point(
            &[corner(1, 0.0, 0.0)],
            true,
            Point::new(0.0, 0.0)
        ));
    }

    /// Non-zero winding: a figure-eight's two lobes both count, and so does
    /// the overlap of a doubled loop (even-odd would exclude it).
    #[test]
    fn a_doubly_wound_loop_is_inside_by_the_non_zero_rule() {
        let mut anchors = square();
        anchors.extend([
            corner(5, 0.0, 0.0),
            corner(6, 10.0, 0.0),
            corner(7, 10.0, 10.0),
            corner(8, 0.0, 10.0),
        ]);
        assert!(contains_point(&anchors, true, Point::new(5.0, 5.0)));
    }
}
