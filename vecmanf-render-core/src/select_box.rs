//! Draws the selection and hover boxes of the Select tool.
//!
//! A selected object's box is dashed (`edit-interaction-polish` criteria 63
//! and 64), fitted per edge so every corner closes; a hovered object's box is
//! solid. An axis-aligned box is snapped to the device pixel grid (criteria 65
//! and 66). Everything is laid out in screen pixels and converted back to
//! document millimetres, so the draw list stays a pure function of its inputs.
//!
//! Each selected or hovered object's own bounding box reaches here as four
//! document-space corners (oriented to the object's own rotation,
//! `object-transform` acceptance criterion 18), computed by
//! `vecmanf_ui_core::oriented_bounds` and passed through by
//! `vecmanf-editor-wasm`: this crate cannot read `vecmanf-ui-core`'s
//! selection or `object_bounds` directly (ADR 0011 §3).

use vecmanf_document_core::{NodeId, Point, ViewTransform};

use crate::color::RgbaColor;
use crate::glyphs::{DrawList, quad_outline, thick_line};
use crate::theme;

/// One object's selection box: its four corners in document space, in
/// order around the perimeter — oriented to the object's own rotation
/// (`vecmanf_ui_core::OrientedBox::document_corners`), which for an
/// unrotated object is the plain axis-aligned box slice 4 shipped.
pub type SelectionBox = [Point; 4];

/// What the Select tool decorates this frame: every currently selected
/// object's own box (plural — a heterogeneous multi-select shows each
/// object's own real box simultaneously, never one merged box,
/// `docs/design-system.md`'s "Mixed-state display on multi-select"
/// extension), plus a hovered-but-not-yet-selected object's box.
#[derive(Debug, Clone, Default)]
pub struct SelectDecorationInput {
    /// Selected objects, each with its own id and box.
    pub selected: Vec<(NodeId, SelectionBox)>,
    /// A hovered, not-yet-selected object's id and box, if any.
    pub hovered: Option<(NodeId, SelectionBox)>,
    /// `window.devicePixelRatio`: device pixels per screen (CSS) pixel. An
    /// axis-aligned box line snaps to whole device pixels with it. A value
    /// that is not a positive finite number, and the default 0, read as 1.
    pub device_pixel_ratio: f64,
}

/// How one box edge is cut into dashes: `count` dashes of `dash_px` with
/// `gap_px` between them, a dash at both ends, so the pattern spans the whole
/// edge and is symmetric about its centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DashFit {
    pub(crate) count: u32,
    pub(crate) dash_px: f64,
    pub(crate) gap_px: f64,
}

/// Slack when comparing a fitted gap with its limits, screen pixels.
const FIT_EPSILON_PX: f64 = 1e-9;

/// How far a box edge may lean off an axis and still count as axis-aligned,
/// screen pixels (a quarter turn leaves a rounding error, never a pixel).
const AXIS_EPSILON_PX: f64 = 1e-6;

/// Cuts an edge of `edge_px` screen pixels into dashes (`edit-interaction-
/// polish` criteria 63 and 64), or `None` when it is drawn solid.
///
/// The nominal pattern is 4 on / 3 off. Among the counts whose dash is exactly
/// 4 and whose gap is 2 to 4 the one with the gap nearest 3 is used. Edges
/// strictly between 12 and 16 and between 20 and 22 px have no such count; the
/// gap is then 2 and the dash flexes, to about 2.7 to 4 px, so the corners
/// still close. An edge under 10 px, or over the cap, is solid.
pub(crate) fn fit_dashes(edge_px: f64) -> Option<DashFit> {
    if edge_px.is_nan()
        || edge_px < theme::SELECTION_BOX_MIN_DASHED_EDGE_PX
        || edge_px > theme::SELECTION_BOX_MAX_DASHED_EDGE_PX
    {
        return None;
    }
    let dash = theme::SELECTION_BOX_DASH_PX;
    let mut best: Option<DashFit> = None;
    let mut count = 2_u32;
    // `count` dashes of exactly `dash` need `count * dash + (count - 1) * gap_min`
    // at most; past that no gap is small enough.
    while f64::from(count) * dash + f64::from(count - 1) * theme::SELECTION_BOX_GAP_MIN_PX
        <= edge_px + FIT_EPSILON_PX
    {
        let gap = (edge_px - f64::from(count) * dash) / f64::from(count - 1);
        let in_range = (theme::SELECTION_BOX_GAP_MIN_PX - FIT_EPSILON_PX
            ..=theme::SELECTION_BOX_GAP_MAX_PX + FIT_EPSILON_PX)
            .contains(&gap);
        let nearer = best.is_none_or(|b| {
            (gap - theme::SELECTION_BOX_GAP_PX).abs()
                < (b.gap_px - theme::SELECTION_BOX_GAP_PX).abs()
        });
        if in_range && nearer {
            best = Some(DashFit {
                count,
                dash_px: dash,
                gap_px: gap,
            });
        }
        count += 1;
    }
    // `count` is now the smallest count whose exact fit would need a gap under
    // the minimum: the flex count, with the minimum gap and a shorter dash.
    best.or_else(|| {
        let gap = theme::SELECTION_BOX_GAP_MIN_PX;
        Some(DashFit {
            count,
            dash_px: (edge_px - f64::from(count - 1) * gap) / f64::from(count),
            gap_px: gap,
        })
    })
}

/// The line width in device pixels of an axis-aligned box line: whole device
/// pixels, at least 1, so that it can cover whole rows and columns.
pub(crate) fn device_line_width(ratio: f64) -> f64 {
    (theme::BOUNDING_BOX_OUTLINE_PX * ratio).round().max(1.0)
}

/// Snaps the centre line `coord_px` (screen pixels) of a line `line_device_px`
/// device pixels wide onto the device pixel grid, so that both its edges fall
/// on pixel boundaries: the centre sits on a pixel boundary for an even width
/// and in a pixel's middle for an odd one. Moves it by at most half a device
/// pixel.
pub(crate) fn snap_to_device(coord_px: f64, ratio: f64, line_device_px: f64) -> f64 {
    let odd = (line_device_px % 2.0).abs() > 0.5;
    let centre_offset = if odd { 0.5 } else { 0.0 };
    ((coord_px * ratio - centre_offset).round() + centre_offset) / ratio
}

type Screen = (f64, f64);

fn effective_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        1.0
    }
}

/// A box in screen pixels, with the width its line is drawn at.
struct ScreenBox {
    corners: [Screen; 4],
    width_px: f64,
}

impl ScreenBox {
    /// `corners` in screen pixels. An axis-aligned box is snapped to the device
    /// pixel grid and takes a whole number of device pixels of width; a rotated
    /// one keeps its true corners and the 1 px width (it can be no crisper).
    fn new(view: ViewTransform, corners: SelectionBox, ratio: f64) -> Self {
        let screen = corners.map(|corner| view.document_to_screen(corner));
        let axis_aligned = (0..4).all(|i| {
            let (a, b) = (screen[i], screen[(i + 1) % 4]);
            (b.0 - a.0).abs() <= AXIS_EPSILON_PX || (b.1 - a.1).abs() <= AXIS_EPSILON_PX
        });
        if !axis_aligned {
            return Self {
                corners: screen,
                width_px: theme::BOUNDING_BOX_OUTLINE_PX,
            };
        }
        let device_width = device_line_width(ratio);
        Self {
            corners: screen.map(|(x, y)| {
                (
                    snap_to_device(x, ratio, device_width),
                    snap_to_device(y, ratio, device_width),
                )
            }),
            width_px: device_width / ratio,
        }
    }

    fn document_corners(&self, view: ViewTransform) -> SelectionBox {
        self.corners.map(|(x, y)| view.screen_to_document(x, y))
    }
}

/// One box edge `from` to `to` (screen pixels) as dashes, or one solid line.
/// The first and the last dash reach half a line width past the corner, so
/// the two edges that meet there cover the whole corner pixel.
fn dashed_edge(
    view: ViewTransform,
    from: Screen,
    to: Screen,
    width_px: f64,
    color: RgbaColor,
) -> DrawList {
    let mut list = DrawList::default();
    let length = (to.0 - from.0).hypot(to.1 - from.1);
    if length <= f64::EPSILON {
        return list;
    }
    let (ux, uy) = ((to.0 - from.0) / length, (to.1 - from.1) / length);
    let at = |along: f64| view.screen_to_document(from.0 + ux * along, from.1 + uy * along);
    let width_mm = width_px / view.scale();
    let half = width_px / 2.0;
    let spans: Vec<(f64, f64)> = match fit_dashes(length) {
        None => vec![(-half, length + half)],
        Some(fit) => (0..fit.count)
            .map(|i| {
                let start = f64::from(i) * (fit.dash_px + fit.gap_px);
                let end = if i + 1 == fit.count {
                    length
                } else {
                    start + fit.dash_px
                };
                (
                    if i == 0 { start - half } else { start },
                    if i + 1 == fit.count { end + half } else { end },
                )
            })
            .collect(),
    };
    for (start, end) in spans {
        list.extend(thick_line(at(start), at(end), width_mm, color));
    }
    list
}

/// Builds the Select tool's decoration geometry for this frame: one dashed
/// `--accent` box per selected object, plus a solid `--accent-hover` box for a
/// hovered-but-unselected one (`docs/design-system.md`'s "Bounding-box
/// selection outline").
#[must_use]
pub fn build(view: ViewTransform, input: &SelectDecorationInput) -> DrawList {
    let ratio = effective_ratio(input.device_pixel_ratio);
    let mut list = DrawList::default();
    for &(_, corners) in &input.selected {
        let screen = ScreenBox::new(view, corners, ratio);
        for i in 0..4 {
            list.extend(dashed_edge(
                view,
                screen.corners[i],
                screen.corners[(i + 1) % 4],
                screen.width_px,
                theme::ACCENT,
            ));
        }
    }
    if let Some((_, corners)) = input.hovered {
        let screen = ScreenBox::new(view, corners, ratio);
        list.extend(quad_outline(
            screen.document_corners(view),
            screen.width_px / view.scale(),
            theme::ACCENT_HOVER,
        ));
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_id() -> NodeId {
        // `NodeId` has no public constructor outside `document-core`; any
        // real one round-tripped through a `Document` is fine here, since
        // these tests never read the id back, only the geometry it keys.
        let document = vecmanf_document_core::Document::new(1);
        document.create_rect(vecmanf_document_core::RectBounds {
            origin: Point::new(0.0, 0.0),
            width: vecmanf_document_core::Length::from_mm(1.0),
            height: vecmanf_document_core::Length::from_mm(1.0),
        })
    }

    fn axis_box(x0: f64, y0: f64, x1: f64, y1: f64) -> SelectionBox {
        [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    /// Acceptance criterion 18 / UX notes: a rotated object's outline is
    /// drawn through its own four (turned) corners, not their axis-
    /// aligned bounds — a 45° diamond's outline reaches its apex at
    /// `(0, -r)` and never the bounding square's corner `(r, -r)`.
    #[test]
    fn a_rotated_selection_box_draws_through_its_own_corners_not_its_bounds() {
        let r = 10.0;
        let diamond: SelectionBox = [
            Point::new(0.0, -r),
            Point::new(r, 0.0),
            Point::new(0.0, r),
            Point::new(-r, 0.0),
        ];
        let input = SelectDecorationInput {
            selected: vec![(fixture_id(), diamond)],
            hovered: None,
            ..SelectDecorationInput::default()
        };
        let list = build(ViewTransform::identity(), &input);
        let reaches = |target: Point| {
            list.triangles
                .iter()
                .any(|v| v.position.vector_to(target).length() < 1.0)
        };
        assert!(reaches(Point::new(0.0, -r)), "apex drawn");
        assert!(
            !reaches(Point::new(r, -r)),
            "bounding-square corner not drawn"
        );
    }

    #[test]
    fn no_selection_and_no_hover_draws_nothing() {
        let list = build(ViewTransform::identity(), &SelectDecorationInput::default());
        assert_eq!(list.triangles.len(), 0);
    }

    #[test]
    fn a_selected_object_draws_a_box() {
        let input = SelectDecorationInput {
            selected: vec![(fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0))],
            hovered: None,
            ..SelectDecorationInput::default()
        };
        let list = build(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }

    #[test]
    fn a_hovered_object_draws_a_box_too() {
        let input = SelectDecorationInput {
            selected: vec![],
            hovered: Some((fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0))),
            ..SelectDecorationInput::default()
        };
        let list = build(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }

    /// Acceptance criterion 17's multi-select UX note: two selected
    /// objects draw two independent boxes, not one merged box — strictly
    /// more geometry than either alone.
    #[test]
    fn two_selected_objects_each_draw_their_own_box() {
        let one = SelectDecorationInput {
            selected: vec![(fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0))],
            hovered: None,
            ..SelectDecorationInput::default()
        };
        let two = SelectDecorationInput {
            selected: vec![
                (fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0)),
                (fixture_id(), axis_box(50.0, 50.0, 60.0, 60.0)),
            ],
            hovered: None,
            ..SelectDecorationInput::default()
        };
        let one_list = build(ViewTransform::identity(), &one);
        let two_list = build(ViewTransform::identity(), &two);
        assert!(two_list.triangle_count() > one_list.triangle_count());
    }

    // ---- dash fitting (criteria 63, 64) -------------------------------

    fn steps(from: f64, to: f64, step: f64) -> impl Iterator<Item = f64> {
        (0..)
            .map(move |i| from + f64::from(i) * step)
            .take_while(move |&x| x <= to)
    }

    fn total(fit: DashFit) -> f64 {
        f64::from(fit.count) * fit.dash_px + f64::from(fit.count - 1) * fit.gap_px
    }

    #[test]
    fn an_edge_under_ten_pixels_is_solid() {
        for length in steps(0.0, 9.99, 0.25) {
            assert_eq!(fit_dashes(length), None, "{length}");
        }
        assert_eq!(fit_dashes(f64::NAN), None);
    }

    /// Every length from 10 to 400 px: a dash at both ends (the fit spans the
    /// whole edge), a dash never over 4 or under 2.5 px, a gap of 2 to 4 px.
    #[test]
    fn every_edge_length_fits_with_a_dash_at_both_ends() {
        for length in steps(10.0, 400.0, 0.25) {
            let fit = fit_dashes(length).expect("fits");
            assert!(fit.count >= 2, "{length}");
            assert!((total(fit) - length).abs() < 1e-9, "{length}: {fit:?}");
            assert!(fit.dash_px <= 4.0 + 1e-9, "{length}: {fit:?}");
            assert!(fit.dash_px >= 2.5, "{length}: {fit:?}");
            assert!(
                (2.0 - 1e-9..=4.0 + 1e-9).contains(&fit.gap_px),
                "{length}: {fit:?}"
            );
        }
    }

    /// Outside the two bands that have no exact fit the dash is exactly 4 px;
    /// inside them the gap is 2 px and the dash flexes (flagged decision 2).
    #[test]
    fn only_the_two_flagged_bands_flex_the_dash() {
        for length in steps(10.0, 400.0, 0.25) {
            let fit = fit_dashes(length).expect("fits");
            let in_band = (length > 12.0 + 1e-9 && length < 16.0 - 1e-9)
                || (length > 20.0 + 1e-9 && length < 22.0 - 1e-9);
            if in_band {
                assert!((fit.gap_px - 2.0).abs() < 1e-9, "{length}: {fit:?}");
                assert!(fit.dash_px < 4.0, "{length}: {fit:?}");
            } else {
                assert!((fit.dash_px - 4.0).abs() < 1e-9, "{length}: {fit:?}");
            }
        }
    }

    /// The customer's V1 is 4 on / 3 off: a long edge keeps a gap near 3, not
    /// the largest gap that fits.
    #[test]
    fn a_long_edge_keeps_a_gap_near_the_nominal_three() {
        for length in steps(100.0, 400.0, 0.25) {
            let fit = fit_dashes(length).expect("fits");
            assert!((fit.gap_px - 3.0).abs() <= 0.5, "{length}: {fit:?}");
        }
        let exact = fit_dashes(4.0 * 15.0 + 3.0 * 14.0).expect("fits");
        assert_eq!(exact.count, 15);
        assert!((exact.gap_px - 3.0).abs() < 1e-9);
    }

    #[test]
    fn the_two_flex_bands_close_their_corners() {
        for length in steps(12.25, 15.75, 0.25).chain(steps(20.25, 21.75, 0.25)) {
            let fit = fit_dashes(length).expect("fits");
            assert!((total(fit) - length).abs() < 1e-9, "{length}: {fit:?}");
            assert!((2.5..4.0).contains(&fit.dash_px), "{length}: {fit:?}");
        }
        // About 2.7 px at the bottom of the first band, about 3.5 at the second.
        assert!((fit_dashes(12.01).expect("fits").dash_px - 2.67).abs() < 0.01);
        assert!((fit_dashes(20.01).expect("fits").dash_px - 3.5).abs() < 0.01);
    }

    // ---- the dashed box (criteria 63, 64, 66, 67) ----------------------

    type Pt = (f64, f64);

    fn selected_input(corners: SelectionBox) -> SelectDecorationInput {
        SelectDecorationInput {
            selected: vec![(fixture_id(), corners)],
            ..SelectDecorationInput::default()
        }
    }

    fn hovered_input(corners: SelectionBox) -> SelectDecorationInput {
        SelectDecorationInput {
            hovered: Some((fixture_id(), corners)),
            ..SelectDecorationInput::default()
        }
    }

    /// An axis-aligned box of `w` by `h` screen pixels at the screen point
    /// `(x, y)`, for a view with `scale` pixels per millimetre and the
    /// document origin at the screen origin.
    fn px_box(x: f64, y: f64, w: f64, h: f64, scale: f64) -> SelectionBox {
        axis_box(x / scale, y / scale, (x + w) / scale, (y + h) / scale)
    }

    /// A box of `w` by `h` screen pixels turned `degrees`, its first corner at `(x, y)`.
    fn turned_box(x: f64, y: f64, w: f64, h: f64, degrees: f64, scale: f64) -> SelectionBox {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let corner = |u: f64, v: f64| {
            Point::new(
                (x + u * cos - v * sin) / scale,
                (y + u * sin + v * cos) / scale,
            )
        };
        [
            corner(0.0, 0.0),
            corner(w, 0.0),
            corner(w, h),
            corner(0.0, h),
        ]
    }

    fn view_at(scale: f64) -> ViewTransform {
        ViewTransform::new(scale, Point::new(0.0, 0.0))
    }

    /// The draw list's triangles in screen pixels.
    fn screen_triangles(list: &DrawList, view: ViewTransform) -> Vec<[Pt; 3]> {
        list.triangles
            .chunks(3)
            .map(|t| [0, 1, 2].map(|i| view.document_to_screen(t[i].position)))
            .collect()
    }

    fn inside(triangle: &[Pt; 3], p: Pt) -> bool {
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

    fn covered(triangles: &[[Pt; 3]], p: Pt) -> bool {
        triangles.iter().any(|t| inside(t, p))
    }

    fn lerp(a: Pt, b: Pt, t: f64) -> Pt {
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }

    /// Criterion 64: every corner is closed, for both edges that meet there,
    /// on edges across the whole range including the two flex bands.
    #[test]
    fn every_corner_of_the_selection_box_is_closed() {
        let view = view_at(2.0);
        let lengths: [f64; 19] = [
            10.0, 11.0, 12.0, 12.5, 13.0, 14.0, 15.0, 15.9, 16.0, 17.0, 20.0, 21.0, 21.9, 22.0,
            24.0, 31.0, 57.0, 100.0, 301.0,
        ];
        for &w in &lengths {
            for &h in &lengths {
                for degrees in [0.0, 30.0, 117.0] {
                    // An axis-aligned box is snapped, so it starts and ends on whole
                    // pixels here and the snap leaves its corners where they are.
                    let (w, h) = if degrees == 0.0 {
                        (w.round(), h.round())
                    } else {
                        (w, h)
                    };
                    let corners = turned_box(40.5, 55.5, w, h, degrees, view.scale());
                    let list = build(view, &selected_input(corners));
                    let triangles = screen_triangles(&list, view);
                    let screen = corners.map(|c| view.document_to_screen(c));
                    for i in 0..4 {
                        let corner = screen[i];
                        let along = screen[(i + 1) % 4];
                        let back = screen[(i + 3) % 4];
                        let edge = |to: Pt| {
                            let len = (to.0 - corner.0).hypot(to.1 - corner.1);
                            lerp(corner, to, 0.25 / len)
                        };
                        assert!(
                            covered(&triangles, corner),
                            "corner {i} of {w}x{h} at {degrees} deg"
                        );
                        // The corner pixel itself, outside both edges' own ends:
                        // only the half-width extension of an end dash covers it.
                        let outward = |to: Pt| {
                            let len = (to.0 - corner.0).hypot(to.1 - corner.1);
                            ((to.0 - corner.0) / len, (to.1 - corner.1) / len)
                        };
                        let (a, b) = (outward(along), outward(back));
                        let outside =
                            (corner.0 - 0.25 * (a.0 + b.0), corner.1 - 0.25 * (a.1 + b.1));
                        assert!(
                            covered(&triangles, outside),
                            "outer corner square {i} of {w}x{h} at {degrees} deg"
                        );
                        assert!(
                            covered(&triangles, edge(along)),
                            "start of the edge leaving corner {i}: {w}x{h} at {degrees}"
                        );
                        assert!(
                            covered(&triangles, edge(back)),
                            "end of the edge entering corner {i}: {w}x{h} at {degrees}"
                        );
                    }
                }
            }
        }
    }

    /// An edge under 10 px is one solid line; a longer one is several dashes.
    #[test]
    fn short_edges_are_solid_and_long_edges_are_dashed() {
        let view = view_at(1.0);
        let short = build(view, &selected_input(px_box(0.5, 0.5, 8.0, 8.0, 1.0)));
        assert_eq!(short.triangle_count(), 8, "four solid edges");
        let long = build(view, &selected_input(px_box(0.5, 0.5, 100.0, 8.0, 1.0)));
        // Two long edges of 100 px (15 dashes each) and two solid short ones.
        assert_eq!(long.triangle_count(), 2 * (15 * 2) + 2 * 2);
    }

    /// Criterion 66: the hover box stays solid, at `--accent-hover`.
    #[test]
    fn the_hover_box_is_solid_and_the_selection_box_is_dashed() {
        let view = view_at(1.0);
        let corners = px_box(0.5, 0.5, 100.0, 100.0, 1.0);
        let hover = build(view, &hovered_input(corners));
        assert_eq!(hover.triangle_count(), 8, "four solid edges");
        assert!(
            hover
                .triangles
                .iter()
                .all(|v| v.color == theme::ACCENT_HOVER)
        );
        let selected = build(view, &selected_input(corners));
        assert!(selected.triangle_count() > hover.triangle_count());
        assert!(selected.triangles.iter().all(|v| v.color == theme::ACCENT));
    }

    /// Criterion 63: each selected object has its own dashed box.
    #[test]
    fn each_selected_object_draws_its_own_dashed_box() {
        let view = view_at(1.0);
        let a = px_box(0.5, 0.5, 60.0, 40.0, 1.0);
        let b = px_box(200.5, 100.5, 80.0, 50.0, 1.0);
        let one = build(view, &selected_input(a)).triangle_count();
        let other = build(view, &selected_input(b)).triangle_count();
        let both = build(
            view,
            &SelectDecorationInput {
                selected: vec![(fixture_id(), a), (fixture_id(), b)],
                ..SelectDecorationInput::default()
            },
        );
        assert_eq!(both.triangle_count(), one + other);
    }

    /// Criterion 63: the pattern is a pure function of the box in screen
    /// pixels, so a pan, a move or a zoom shifts it rigidly: the same box
    /// shape at another position and zoom gives the same screen geometry.
    #[test]
    fn the_dash_pattern_is_rigid_under_translation_and_zoom() {
        let (w, h, degrees) = (137.0, 61.0, 30.0);
        let reference_view = view_at(1.0);
        let reference = screen_triangles(
            &build(
                reference_view,
                &selected_input(turned_box(10.0, 20.0, w, h, degrees, 1.0)),
            ),
            reference_view,
        );
        for scale in [0.25, 2.0, 7.5] {
            for (dx, dy) in [(0.0, 0.0), (300.0, -80.0), (-41.7, 1234.5)] {
                // The same screen geometry, shifted by (dx, dy) screen pixels, drawn
                // at another zoom and over a panned view (the view's origin).
                let (pan_x, pan_y) = (7.3, -2.1);
                let view = ViewTransform::new(scale, Point::new(pan_x / scale, pan_y / scale));
                let corners =
                    turned_box(10.0 + dx + pan_x, 20.0 + dy + pan_y, w, h, degrees, scale);
                let triangles = screen_triangles(&build(view, &selected_input(corners)), view);
                assert_eq!(triangles.len(), reference.len(), "scale {scale}");
                for (got, want) in triangles.iter().zip(&reference) {
                    for (g, r) in got.iter().zip(want) {
                        assert!(
                            (g.0 - dx - r.0).abs() < 1e-6 && (g.1 - dy - r.1).abs() < 1e-6,
                            "scale {scale}, offset ({dx}, {dy}): {g:?} vs {r:?}"
                        );
                    }
                }
            }
        }
    }

    /// A rotated box's dashes follow its own edges, 1 px wide.
    #[test]
    fn a_rotated_boxs_dashes_follow_its_edges() {
        let view = view_at(1.0);
        let corners = turned_box(50.0, 50.0, 90.0, 40.0, 30.0, 1.0);
        let screen = corners.map(|c| view.document_to_screen(c));
        let triangles = screen_triangles(&build(view, &selected_input(corners)), view);
        let distance_to_edge = |p: Pt, a: Pt, b: Pt| {
            let (ex, ey) = (b.0 - a.0, b.1 - a.1);
            let t = (((p.0 - a.0) * ex + (p.1 - a.1) * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
            (p.0 - a.0 - t * ex).hypot(p.1 - a.1 - t * ey)
        };
        for vertex in triangles.iter().flatten() {
            let nearest = (0..4)
                .map(|i| distance_to_edge(*vertex, screen[i], screen[(i + 1) % 4]))
                .fold(f64::MAX, f64::min);
            // Half the width off the edge, and at a corner half a width along it.
            assert!(
                nearest <= 0.5 * std::f64::consts::SQRT_2 + 1e-6,
                "{vertex:?}: {nearest}"
            );
        }
    }

    #[test]
    fn equal_inputs_give_equal_draw_lists() {
        let view = view_at(3.0);
        let input = selected_input(turned_box(33.3, 71.1, 90.0, 40.0, 12.0, 3.0));
        assert_eq!(build(view, &input).triangles, build(view, &input).triangles);
    }

    /// Criterion 67: the dashes knock nothing out; every triangle is the
    /// accent colour and nothing is drawn in the gaps.
    #[test]
    fn the_dashes_leave_the_gaps_empty() {
        let view = view_at(1.0);
        let triangles = screen_triangles(
            &build(view, &selected_input(px_box(10.5, 10.5, 100.0, 60.0, 1.0))),
            view,
        );
        // The top edge is 100 px: 15 dashes at 4 on / 3 off from x = 10.5; the
        // middle of the first gap is uncovered, the middle of the first dash is.
        assert!(covered(&triangles, (12.5, 10.5)));
        assert!(!covered(&triangles, (15.5, 10.5)));
        assert!(covered(&triangles, (19.5, 10.5)));
    }

    // ---- the pixel snap (criteria 65, 66) ------------------------------

    #[test]
    fn the_snap_moves_a_line_at_most_half_a_device_pixel_onto_the_grid() {
        for ratio in [1.0, 1.25, 1.5, 2.0, 3.0] {
            let width = device_line_width(ratio);
            for x in steps(-20.0, 20.0, 0.037) {
                let snapped = snap_to_device(x, ratio, width);
                assert!(
                    (snapped - x).abs() * ratio <= 0.5 + 1e-9,
                    "ratio {ratio}, x {x}: moved to {snapped}"
                );
                // The line's edges, centre -/+ half the width, are whole device pixels.
                let left = (snapped - width / ratio / 2.0) * ratio;
                assert!(
                    (left - left.round()).abs() < 1e-6,
                    "ratio {ratio}, x {x}: {left}"
                );
            }
        }
    }

    #[test]
    fn the_axis_aligned_line_is_a_whole_number_of_device_pixels_wide() {
        assert!((device_line_width(1.0) - 1.0).abs() < 1e-12);
        assert!((device_line_width(1.25) - 1.0).abs() < 1e-12);
        assert!((device_line_width(1.5) - 2.0).abs() < 1e-12);
        assert!((device_line_width(2.0) - 2.0).abs() < 1e-12);
        assert!((device_line_width(3.0) - 3.0).abs() < 1e-12);
        assert!((device_line_width(0.4) - 1.0).abs() < 1e-12);
    }

    /// Criterion 65: at every ratio and position the hover box's four lines
    /// cover whole device rows and columns (both long edges of each quad lie on
    /// device pixel boundaries), so none is drawn as two half-covered rows.
    #[test]
    fn an_axis_aligned_box_covers_whole_device_pixel_rows_and_columns() {
        for ratio in [1.0, 1.25, 1.5, 2.0, 3.0] {
            for scale in [0.7, 1.0, 2.0, 3.37] {
                for (x, y) in [(10.0, 20.0), (10.3, 20.7), (33.49, 5.51), (-7.2, 91.9)] {
                    let view = ViewTransform::new(scale, Point::new(0.3, -0.2));
                    let corners = [
                        view.screen_to_document(x, y),
                        view.screen_to_document(x + 87.3, y),
                        view.screen_to_document(x + 87.3, y + 41.1),
                        view.screen_to_document(x, y + 41.1),
                    ];
                    let input = SelectDecorationInput {
                        hovered: Some((fixture_id(), corners)),
                        device_pixel_ratio: ratio,
                        ..SelectDecorationInput::default()
                    };
                    let list = build(view, &input);
                    assert_eq!(list.triangle_count(), 8);
                    for vertex in screen_triangles(&list, view).iter().flatten() {
                        // A vertex of a vertical quad sits on a device column
                        // boundary, one of a horizontal quad on a row boundary.
                        let on = |v: f64| (v * ratio - (v * ratio).round()).abs() < 1e-6;
                        assert!(
                            on(vertex.0) || on(vertex.1),
                            "ratio {ratio}, scale {scale}, at ({x}, {y}): {vertex:?}"
                        );
                    }
                    // The left line is exactly `device_line_width` device pixels wide.
                    let xs: Vec<f64> = screen_triangles(&list, view)
                        .iter()
                        .flatten()
                        .map(|p| p.0 * ratio)
                        .collect();
                    let min = xs.iter().copied().fold(f64::MAX, f64::min);
                    let max_left = xs
                        .iter()
                        .copied()
                        .filter(|&v| v < min + device_line_width(ratio) + 1e-6)
                        .fold(f64::MIN, f64::max);
                    assert!(
                        (max_left - min - device_line_width(ratio)).abs() < 1e-6,
                        "ratio {ratio}: left line {min}..{max_left}"
                    );
                }
            }
        }
    }

    /// The selection box is pixel-aligned across its width too, and its
    /// corners stay closed after the snap.
    #[test]
    fn the_dashed_box_lines_sit_on_whole_device_pixels() {
        for ratio in [1.0, 1.5, 2.0] {
            let view = view_at(2.0);
            let input = SelectDecorationInput {
                selected: vec![(fixture_id(), px_box(10.3, 20.7, 100.0, 60.0, 2.0))],
                device_pixel_ratio: ratio,
                ..SelectDecorationInput::default()
            };
            let list = build(view, &input);
            for vertex in screen_triangles(&list, view).iter().flatten() {
                let on = |v: f64| (v * ratio - (v * ratio).round()).abs() < 1e-6;
                // Across the line a whole device pixel; along it the dash ends
                // are fractional.
                assert!(on(vertex.0) || on(vertex.1), "ratio {ratio}: {vertex:?}");
            }
        }
    }

    /// A rotated box is not snapped: it keeps its true corners and 1 px width.
    #[test]
    fn a_rotated_box_is_not_snapped() {
        let view = view_at(1.0);
        let corners = turned_box(50.3, 50.7, 90.0, 40.0, 30.0, 1.0);
        let at = |ratio: f64| {
            build(
                view,
                &SelectDecorationInput {
                    hovered: Some((fixture_id(), corners)),
                    device_pixel_ratio: ratio,
                    ..SelectDecorationInput::default()
                },
            )
        };
        assert_eq!(at(1.0).triangles, at(2.0).triangles);
        let reference = quad_outline(corners, 1.0, theme::ACCENT_HOVER);
        assert_eq!(at(1.0).triangles, reference.triangles);
    }

    /// A box that is axis-aligned only because it was turned a quarter turn is
    /// snapped like any other.
    #[test]
    fn a_quarter_turned_box_counts_as_axis_aligned() {
        let view = view_at(1.0);
        let corners = turned_box(100.0, 20.3, 50.0, 30.0, 90.0, 1.0);
        let hover = build(view, &hovered_input(corners));
        assert_ne!(
            hover.triangles,
            quad_outline(corners, 1.0, theme::ACCENT_HOVER).triangles
        );
    }

    /// A ratio that is not a positive finite number reads as 1.
    #[test]
    fn a_nonsense_ratio_reads_as_one() {
        let view = view_at(1.0);
        let corners = px_box(10.3, 20.7, 50.0, 30.0, 1.0);
        let at = |ratio: f64| {
            build(
                view,
                &SelectDecorationInput {
                    selected: vec![(fixture_id(), corners)],
                    device_pixel_ratio: ratio,
                    ..SelectDecorationInput::default()
                },
            )
        };
        let one = at(1.0);
        for bad in [0.0, -2.0, f64::NAN, f64::INFINITY] {
            assert_eq!(at(bad).triangles, one.triangles, "{bad}");
        }
    }

    /// An edge longer than the cap is one solid line (bounds the draw list).
    #[test]
    fn an_absurdly_long_edge_is_solid() {
        let view = view_at(1.0);
        let list = build(view, &selected_input(px_box(0.5, 0.5, 100_000.0, 8.0, 1.0)));
        assert_eq!(list.triangle_count(), 8);
    }
}
