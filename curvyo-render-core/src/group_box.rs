//! Draws the group box of a multi-selection and the lighter member boxes under it
//! (`specs/0019-multi-object-transform/`, criteria 4, 5, 8, 13; `docs/design-system.md`,
//! rows "Group selection box" and "Member box"). Split out of [`crate::select_box`], which
//! owns the dash fitting and the pixel snapping both boxes use.

use curvyo_document_core::{Point, ViewTransform};

use crate::glyphs::{DrawList, quad_outline};
use crate::select_box::{
    Screen, ScreenBox, SelectionBox, dashed_edge, effective_ratio, guide_cover,
};
use crate::theme;

/// How a group box is drawn (criteria 8 and 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBoxKind {
    /// Both axes have an extent: the dashed box.
    Box,
    /// One axis has none: one dashed line, drawn once.
    Line,
    /// Neither has: a 6 px square outline marks the point.
    Point,
}

/// What the group box of a multi-selection decorates this frame.
#[derive(Debug, Clone)]
pub struct GroupDecorationInput {
    /// The group box's four corners in document space, in order around the
    /// perimeter (turned during a rotate drag).
    pub corners: SelectionBox,
    /// A box, a line or a point.
    pub kind: GroupBoxKind,
    /// Each selected object's own box. The caller leaves the list empty above 500
    /// selected objects (criterion 5).
    pub members: Vec<SelectionBox>,
    /// The canvas size in screen pixels, for skipping member boxes outside the
    /// viewport; `(0, 0)` skips none.
    pub canvas_px: (f64, f64),
    /// `window.devicePixelRatio`; not a positive finite number reads as 1.
    pub device_pixel_ratio: f64,
    /// The two end points of the skew fixed-line guide while a skew drag runs:
    /// the group box leaves its dashes off the edge the guide covers.
    pub skew_guide: Option<(Point, Point)>,
}

/// Whether a member box is drawn: not under 6 px on both of its sides, and not
/// entirely outside the viewport (criterion 4).
fn member_is_drawn(member: &ScreenBox, canvas_px: (f64, f64)) -> bool {
    let c = member.corners;
    let side = |a: Screen, b: Screen| (b.0 - a.0).hypot(b.1 - a.1);
    let (along, across) = (side(c[0], c[1]), side(c[1], c[2]));
    if along < theme::MEMBER_BOX_MIN_SIDE_PX && across < theme::MEMBER_BOX_MIN_SIDE_PX {
        return false;
    }
    let (width, height) = canvas_px;
    if width > 0.0 && height > 0.0 {
        let left = c.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let right = c.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let top = c.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let bottom = c.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        if right < 0.0 || left > width || bottom < 0.0 || top > height {
            return false;
        }
    }
    true
}

/// The edges of the group box that carry a line: all four of a box, the one
/// edge of a line (the longest, so the two coincident ones are drawn once), none
/// of a point.
fn group_edges(group: &ScreenBox, kind: GroupBoxKind) -> Vec<(Screen, Screen)> {
    let edge = |i: usize| (group.corners[i], group.corners[(i + 1) % 4]);
    match kind {
        GroupBoxKind::Box => (0..4).map(edge).collect(),
        GroupBoxKind::Line => {
            let length = |(a, b): (Screen, Screen)| (b.0 - a.0).hypot(b.1 - a.1);
            (0..4)
                .map(edge)
                .max_by(|a, b| length(*a).total_cmp(&length(*b)))
                .into_iter()
                .collect()
        }
        GroupBoxKind::Point => Vec::new(),
    }
}

/// The square outline that marks a group box that is a single point
/// (criterion 13): 6 px, solid, 1 px, with the casing.
fn point_marker(view: ViewTransform, at: Point, ratio: f64) -> DrawList {
    let half = theme::GROUP_POINT_MARKER_PX / 2.0 / view.scale();
    let corners = [
        Point::new(at.x - half, at.y - half),
        Point::new(at.x + half, at.y - half),
        Point::new(at.x + half, at.y + half),
        Point::new(at.x - half, at.y + half),
    ];
    let line = crate::select_box::device_line_width(ratio) / ratio / view.scale();
    let mut list = quad_outline(
        corners,
        line * theme::CASING_WIDTH_FACTOR,
        theme::SELECTION_CASING,
    );
    list.extend(quad_outline(corners, line, theme::ACCENT));
    list
}

/// The member boxes then the group box.
pub(crate) fn build(view: ViewTransform, input: &GroupDecorationInput) -> DrawList {
    let ratio = effective_ratio(input.device_pixel_ratio);
    let guide = input
        .skew_guide
        .map(|(a, b)| (view.document_to_screen(a), view.document_to_screen(b)));
    let mut list = DrawList::default();
    let group_screen = ScreenBox::new(view, input.corners, ratio);
    let edges = group_edges(&group_screen, input.kind);
    let coincident_px = theme::MEMBER_BOX_COINCIDENT_DEVICE_PX / ratio;
    let members: Vec<ScreenBox> = input
        .members
        .iter()
        .map(|&corners| ScreenBox::new(view, corners, ratio))
        .filter(|member| member_is_drawn(member, input.canvas_px))
        .collect();
    for (color, factor) in [
        (theme::MEMBER_BOX_CASING, theme::CASING_WIDTH_FACTOR),
        (theme::MEMBER_BOX, 1.0),
    ] {
        for member in &members {
            for i in 0..4 {
                let (from, to) = (member.corners[i], member.corners[(i + 1) % 4]);
                let cuts: Vec<_> = edges
                    .iter()
                    .filter_map(|&edge| guide_cover(from, to, edge, coincident_px))
                    .collect();
                list.extend(dashed_edge(
                    view,
                    from,
                    to,
                    member.width_px,
                    &cuts,
                    color,
                    factor,
                ));
            }
        }
    }
    if input.kind == GroupBoxKind::Point {
        let at = view.screen_to_document(group_screen.corners[0].0, group_screen.corners[0].1);
        list.extend(point_marker(view, at, ratio));
        return list;
    }
    for (color, factor) in [
        (theme::SELECTION_CASING, theme::CASING_WIDTH_FACTOR),
        (theme::ACCENT, 1.0),
    ] {
        for &(from, to) in &edges {
            let cuts: Vec<_> = guide
                .and_then(|g| guide_cover(from, to, g, theme::SELECTION_BOX_GUIDE_TOLERANCE_PX))
                .into_iter()
                .collect();
            list.extend(dashed_edge(
                view,
                from,
                to,
                group_screen.width_px,
                &cuts,
                color,
                factor,
            ));
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::RgbaColor;

    type Pt = (f64, f64);

    fn view() -> ViewTransform {
        ViewTransform::new(1.0, Point::new(0.0, 0.0))
    }

    fn axis_box(x0: f64, y0: f64, x1: f64, y1: f64) -> SelectionBox {
        [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    fn input(
        corners: SelectionBox,
        kind: GroupBoxKind,
        members: Vec<SelectionBox>,
    ) -> GroupDecorationInput {
        GroupDecorationInput {
            corners,
            kind,
            members,
            canvas_px: (0.0, 0.0),
            device_pixel_ratio: 1.0,
            skew_guide: None,
        }
    }

    fn inside(triangle: [Pt; 3], p: Pt) -> bool {
        let side = |a: Pt, b: Pt| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
        let (d1, d2, d3) = (
            side(triangle[0], triangle[1]),
            side(triangle[1], triangle[2]),
            side(triangle[2], triangle[0]),
        );
        let negative = d1 < -1e-9 || d2 < -1e-9 || d3 < -1e-9;
        let positive = d1 > 1e-9 || d2 > 1e-9 || d3 > 1e-9;
        !(negative && positive)
    }

    /// Whether a triangle of `color` covers the screen point `p`.
    fn inked(list: &DrawList, color: RgbaColor, p: Pt) -> bool {
        list.triangles.chunks(3).any(|t| {
            t[0].color == color
                && inside(
                    [0, 1, 2].map(|i| view().document_to_screen(t[i].position)),
                    p,
                )
        })
    }

    fn any_ink(list: &DrawList, color: RgbaColor) -> bool {
        list.triangles.iter().any(|v| v.color == color)
    }

    /// Criterion 4: the member box is lighter than the group box.
    #[test]
    fn the_group_box_is_accent_and_the_members_are_the_lighter_member_colour() {
        let group = axis_box(0.5, 0.5, 100.5, 60.5);
        let member = axis_box(10.5, 10.5, 40.5, 40.5);
        let list = build(view(), &input(group, GroupBoxKind::Box, vec![member]));
        assert!(any_ink(&list, theme::ACCENT));
        assert!(any_ink(&list, theme::MEMBER_BOX));
        assert!(any_ink(&list, theme::MEMBER_BOX_CASING));
    }

    /// Criterion 4: the part of a member edge on a group edge is not drawn; one
    /// edge off the group edge is.
    #[test]
    fn a_member_edge_on_a_group_edge_is_not_drawn() {
        let group = axis_box(0.5, 0.5, 100.5, 60.5);
        // A member whose left edge is the group's left edge.
        let member = axis_box(0.5, 10.5, 40.5, 40.5);
        let list = build(view(), &input(group, GroupBoxKind::Box, vec![member]));
        let member_ink_at = |p: Pt| inked(&list, theme::MEMBER_BOX, p);
        let left_edge_ink = (11..40).any(|y| member_ink_at((0.5, f64::from(y) + 0.5)));
        let right_edge_ink = (11..40).any(|y| member_ink_at((40.5, f64::from(y) + 0.5)));
        assert!(!left_edge_ink, "the group box is the only line there");
        assert!(right_edge_ink, "the free edge is drawn");
    }

    /// Criterion 4: a member under 6 px on both sides, and one outside the
    /// viewport, are not drawn; a member that is small on one side only is.
    #[test]
    fn small_and_off_screen_members_are_not_drawn() {
        let group = axis_box(0.5, 0.5, 200.5, 100.5);
        let tiny = axis_box(50.5, 50.5, 55.5, 55.5);
        let list = build(view(), &input(group, GroupBoxKind::Box, vec![tiny]));
        assert!(!any_ink(&list, theme::MEMBER_BOX));
        let thin = axis_box(50.5, 50.5, 150.5, 54.5);
        let list = build(view(), &input(group, GroupBoxKind::Box, vec![thin]));
        assert!(any_ink(&list, theme::MEMBER_BOX), "long on one side");
        let off = axis_box(300.5, 50.5, 340.5, 90.5);
        let mut with_canvas = input(group, GroupBoxKind::Box, vec![off]);
        with_canvas.canvas_px = (250.0, 150.0);
        assert!(!any_ink(&build(view(), &with_canvas), theme::MEMBER_BOX));
        with_canvas.canvas_px = (0.0, 0.0);
        assert!(any_ink(&build(view(), &with_canvas), theme::MEMBER_BOX));
    }

    /// Criterion 8: a box flat in one axis is one line, drawn once, not two
    /// coincident ones (the same ink as one edge of the box).
    #[test]
    fn a_flat_group_box_is_one_line() {
        let line = [
            Point::new(10.5, 20.5),
            Point::new(110.5, 20.5),
            Point::new(110.5, 20.5),
            Point::new(10.5, 20.5),
        ];
        let one = build(view(), &input(line, GroupBoxKind::Line, vec![]));
        let full = build(
            view(),
            &input(axis_box(10.5, 20.5, 110.5, 80.5), GroupBoxKind::Box, vec![]),
        );
        assert!(any_ink(&one, theme::ACCENT));
        // A vertical line: the same single edge, along y.
        let vertical = [
            Point::new(10.5, 20.5),
            Point::new(10.5, 20.5),
            Point::new(10.5, 120.5),
            Point::new(10.5, 120.5),
        ];
        let two = build(view(), &input(vertical, GroupBoxKind::Line, vec![]));
        assert_eq!(
            one.triangles.len(),
            two.triangles.len(),
            "a horizontal and a vertical line of one length draw the same"
        );
        assert!(one.triangles.len() < full.triangles.len() / 3);
    }

    /// Criterion 13: one point is a 6 px square outline and no dashes.
    #[test]
    fn a_point_group_is_a_six_pixel_square() {
        let at = Point::new(50.5, 50.5);
        let list = build(view(), &input([at; 4], GroupBoxKind::Point, vec![]));
        assert!(
            inked(&list, theme::ACCENT, (47.5, 50.5)),
            "left side of the square"
        );
        assert!(inked(&list, theme::ACCENT, (53.5, 50.5)), "right side");
        assert!(!inked(&list, theme::ACCENT, (50.5, 50.5)), "hollow");
    }

    /// Criterion 5: an empty member list (the caller's 500 rule) draws the group box alone.
    #[test]
    fn no_members_draw_the_group_box_alone() {
        let group = axis_box(0.5, 0.5, 100.5, 60.5);
        let list = build(view(), &input(group, GroupBoxKind::Box, vec![]));
        assert!(any_ink(&list, theme::ACCENT));
        assert!(!any_ink(&list, theme::MEMBER_BOX));
    }

    /// The skew guide covers the group edge it lies on: no dashes there.
    #[test]
    fn the_skew_guide_replaces_the_dashes_of_the_group_edge_it_covers() {
        let group = axis_box(0.5, 0.5, 100.5, 60.5);
        let mut with_guide = input(group, GroupBoxKind::Box, vec![]);
        let plain = build(view(), &with_guide);
        with_guide.skew_guide = Some((Point::new(-16.0, 60.5), Point::new(116.5, 60.5)));
        let guided = build(view(), &with_guide);
        assert!(guided.triangles.len() < plain.triangles.len());
        let bottom_ink =
            (1..100).any(|x| inked(&guided, theme::ACCENT, (f64::from(x) + 0.5, 60.5)));
        assert!(!bottom_ink);
    }
}
