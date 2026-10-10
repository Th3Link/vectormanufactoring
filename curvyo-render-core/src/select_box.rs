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
//! `curvyo_ui_core::oriented_bounds` and passed through by
//! `curvyo-editor-wasm`: this crate cannot read `curvyo-ui-core`'s
//! selection or `object_bounds` directly (ADR 0011 §3).

use curvyo_document_core::{NodeId, Point, ViewTransform};

use crate::color::RgbaColor;
use crate::glyphs::{DrawList, quad_outline, thick_line};
use crate::theme;

/// One object's selection box: its four corners in document space, in
/// order around the perimeter — oriented to the object's own rotation
/// (`curvyo_ui_core::OrientedBox::document_corners`), which for an
/// unrotated object is the plain axis-aligned box slice 4 shipped.
pub type SelectionBox = [Point; 4];

/// What the Select tool decorates this frame: every currently selected
/// object's own box (plural — a heterogeneous multi-select shows each
/// object's own real box simultaneously, `docs/design-system.md`'s
/// "Mixed-state display on multi-select" extension; the group box around a
/// multi-selection is drawn on top of these by the `group_box` module, which
/// `multi-object-transform` added), plus a hovered-but-not-yet-selected
/// object's box.
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
    /// The two end points of the skew fixed-line guide while a skew drag runs
    /// (`edit-interaction-polish` criterion 68). A selected box's edge that
    /// lies on this line is not drawn where the guide covers it: the guide
    /// replaces the box's own dashes there, so it reads alone.
    pub skew_guide: Option<(Point, Point)>,
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

pub(crate) type Screen = (f64, f64);

pub(crate) fn effective_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        1.0
    }
}

/// A guide line `from` to `to` (document space) as it is drawn: snapped to the
/// device pixel grid with the box's own rule when it runs along a screen axis
/// (the line on whole device pixels, the ends on the grid, a whole number of
/// device pixels wide), else the true line at 1 px (a rotated line can be no
/// crisper). Returns the end points and the width in screen pixels.
pub(crate) fn snap_guide_line(
    view: ViewTransform,
    from: Point,
    to: Point,
    ratio: f64,
) -> (Point, Point, f64) {
    let ratio = effective_ratio(ratio);
    let (a, b) = (view.document_to_screen(from), view.document_to_screen(to));
    let horizontal = (b.1 - a.1).abs() <= AXIS_EPSILON_PX;
    let vertical = (b.0 - a.0).abs() <= AXIS_EPSILON_PX;
    if !horizontal && !vertical {
        return (from, to, theme::BOUNDING_BOX_OUTLINE_PX);
    }
    let width = device_line_width(ratio);
    let on_grid = |v: f64| (v * ratio).round() / ratio;
    let across = |v: f64| snap_to_device(v, ratio, width);
    let place = |(x, y): Screen| {
        let (x, y) = if horizontal {
            (on_grid(x), across(y))
        } else {
            (across(x), on_grid(y))
        };
        view.screen_to_document(x, y)
    };
    (place(a), place(b), width / ratio)
}

/// A box in screen pixels, with the width its line is drawn at.
pub(crate) struct ScreenBox {
    pub(crate) corners: [Screen; 4],
    pub(crate) width_px: f64,
}

impl ScreenBox {
    /// `corners` in screen pixels. An axis-aligned box is snapped to the device
    /// pixel grid and takes a whole number of device pixels of width; a rotated
    /// one keeps its true corners and the 1 px width (it can be no crisper).
    pub(crate) fn new(view: ViewTransform, corners: SelectionBox, ratio: f64) -> Self {
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

/// The spans `(start, end)` along an edge of `length` screen pixels that carry
/// a line: the dashes, or one solid span. The first and the last reach half a
/// line width past the corner, so the two edges that meet there cover the
/// whole corner pixel.
fn edge_spans(length: f64, width_px: f64) -> Vec<(f64, f64)> {
    let half = width_px / 2.0;
    match fit_dashes(length) {
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
    }
}

/// `spans` without the part inside `(from, to)` (an interval along the same edge).
fn cut_span_interval(spans: Vec<(f64, f64)>, (from, to): (f64, f64)) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(spans.len() + 1);
    for (start, end) in spans {
        if end <= from || start >= to {
            out.push((start, end));
            continue;
        }
        if start < from {
            out.push((start, from));
        }
        if end > to {
            out.push((to, end));
        }
    }
    out
}

/// Where the skew guide `guide` (screen pixels) covers the edge `from` to `to`:
/// the interval of the guide along the edge, if both edge ends lie on the
/// guide's line.
pub(crate) fn guide_cover(
    from: Screen,
    to: Screen,
    guide: (Screen, Screen),
    tolerance_px: f64,
) -> Option<(f64, f64)> {
    let length = (to.0 - from.0).hypot(to.1 - from.1);
    let guide_length = (guide.1.0 - guide.0.0).hypot(guide.1.1 - guide.0.1);
    if length <= f64::EPSILON || guide_length <= f64::EPSILON {
        return None;
    }
    let (ux, uy) = ((to.0 - from.0) / length, (to.1 - from.1) / length);
    let (gx, gy) = (
        (guide.1.0 - guide.0.0) / guide_length,
        (guide.1.1 - guide.0.1) / guide_length,
    );
    // Perpendicular distance of a point from the guide's line.
    let distance = |p: Screen| ((p.0 - guide.0.0) * gy - (p.1 - guide.0.1) * gx).abs();
    if distance(from) > tolerance_px || distance(to) > tolerance_px {
        return None;
    }
    let along = |p: Screen| (p.0 - from.0) * ux + (p.1 - from.1) * uy;
    let (a, b) = (along(guide.0), along(guide.1));
    Some((a.min(b), a.max(b)))
}

/// One box edge `from` to `to` (screen pixels) as dashes, or one solid line,
/// minus the interval `cut` along it where the skew guide draws instead.
pub(crate) fn dashed_edge(
    view: ViewTransform,
    from: Screen,
    to: Screen,
    width_px: f64,
    cuts: &[(f64, f64)],
    color: RgbaColor,
    thickness_factor: f64,
) -> DrawList {
    let mut list = DrawList::default();
    let length = (to.0 - from.0).hypot(to.1 - from.1);
    if length <= f64::EPSILON {
        return list;
    }
    let (ux, uy) = ((to.0 - from.0) / length, (to.1 - from.1) / length);
    let at = |along: f64| view.screen_to_document(from.0 + ux * along, from.1 + uy * along);
    // The dashes are fitted to the line width; a casing keeps the same dashes
    // and is only thicker across.
    let width_mm = width_px * thickness_factor / view.scale();
    let mut spans = edge_spans(length, width_px);
    for &cut in cuts {
        spans = cut_span_interval(spans, cut);
    }
    for (start, end) in spans {
        list.extend(thick_line(at(start), at(end), width_mm, color));
    }
    list
}

/// Builds the Select tool's decoration geometry for this frame: one dashed
/// `--accent` box per selected object, plus a solid `--hover-box` box for a
/// hovered-but-unselected one (`docs/design-system.md`'s "Bounding-box
/// selection outline"). Both are drawn over a white casing one line width
/// wider on each side (`0007` criterion 40): under the dashes only for the
/// selected box, so their rhythm and pixel snapping are unchanged. All
/// casings of the selected boxes come before any dash, so a casing never
/// covers a neighbouring dash.
#[must_use]
pub fn build(view: ViewTransform, input: &SelectDecorationInput) -> DrawList {
    let ratio = effective_ratio(input.device_pixel_ratio);
    let mut list = DrawList::default();
    let guide = input
        .skew_guide
        .map(|(a, b)| (view.document_to_screen(a), view.document_to_screen(b)));
    for (color, factor) in [
        (theme::SELECTION_CASING, theme::CASING_WIDTH_FACTOR),
        (theme::ACCENT, 1.0),
    ] {
        for &(_, corners) in &input.selected {
            let screen = ScreenBox::new(view, corners, ratio);
            for i in 0..4 {
                let (from, to) = (screen.corners[i], screen.corners[(i + 1) % 4]);
                let cut: Vec<_> = guide
                    .and_then(|g| guide_cover(from, to, g, theme::SELECTION_BOX_GUIDE_TOLERANCE_PX))
                    .into_iter()
                    .collect();
                list.extend(dashed_edge(
                    view,
                    from,
                    to,
                    screen.width_px,
                    &cut,
                    color,
                    factor,
                ));
            }
        }
    }
    if let Some((_, corners)) = input.hovered {
        let screen = ScreenBox::new(view, corners, ratio);
        let corners = screen.document_corners(view);
        let width_mm = screen.width_px / view.scale();
        list.extend(quad_outline(
            corners,
            width_mm * theme::CASING_WIDTH_FACTOR,
            theme::HOVER_BOX_CASING,
        ));
        list.extend(quad_outline(corners, width_mm, theme::HOVER_BOX));
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`build`] without the casings: the lines the older tests measure.
    fn build_lines(view: ViewTransform, input: &SelectDecorationInput) -> DrawList {
        let all = build(view, input);
        DrawList::from_triangles(
            all.triangles
                .into_iter()
                .filter(|v| {
                    v.color != theme::SELECTION_CASING && v.color != theme::HOVER_BOX_CASING
                })
                .collect(),
        )
    }

    fn fixture_id() -> NodeId {
        // `NodeId` has no public constructor outside `document-core`; any
        // real one round-tripped through a `Document` is fine here, since
        // these tests never read the id back, only the geometry it keys.
        let document = curvyo_document_core::Document::new(1);
        document.create_rect(curvyo_document_core::RectBounds {
            origin: Point::new(0.0, 0.0),
            width: curvyo_document_core::Length::from_mm(1.0),
            height: curvyo_document_core::Length::from_mm(1.0),
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
        let list = build_lines(ViewTransform::identity(), &input);
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
        let list = build_lines(ViewTransform::identity(), &SelectDecorationInput::default());
        assert_eq!(list.triangles.len(), 0);
    }

    #[test]
    fn a_selected_object_draws_a_box() {
        let input = SelectDecorationInput {
            selected: vec![(fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0))],
            hovered: None,
            ..SelectDecorationInput::default()
        };
        let list = build_lines(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }

    #[test]
    fn a_hovered_object_draws_a_box_too() {
        let input = SelectDecorationInput {
            selected: vec![],
            hovered: Some((fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0))),
            ..SelectDecorationInput::default()
        };
        let list = build_lines(ViewTransform::identity(), &input);
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
        let one_list = build_lines(ViewTransform::identity(), &one);
        let two_list = build_lines(ViewTransform::identity(), &two);
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
                    let list = build_lines(view, &selected_input(corners));
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
        let short = build_lines(view, &selected_input(px_box(0.5, 0.5, 8.0, 8.0, 1.0)));
        assert_eq!(short.triangle_count(), 8, "four solid edges");
        let long = build_lines(view, &selected_input(px_box(0.5, 0.5, 100.0, 8.0, 1.0)));
        // Two long edges of 100 px (15 dashes each) and two solid short ones.
        assert_eq!(long.triangle_count(), 2 * (15 * 2) + 2 * 2);
    }

    /// Criterion 66: the hover box stays solid, at `--accent-hover`.
    #[test]
    fn the_hover_box_is_solid_and_the_selection_box_is_dashed() {
        let view = view_at(1.0);
        let corners = px_box(0.5, 0.5, 100.0, 100.0, 1.0);
        let hover = build_lines(view, &hovered_input(corners));
        assert_eq!(hover.triangle_count(), 8, "four solid edges");
        assert!(hover.triangles.iter().all(|v| v.color == theme::HOVER_BOX));
        let selected = build_lines(view, &selected_input(corners));
        assert!(selected.triangle_count() > hover.triangle_count());
        assert!(selected.triangles.iter().all(|v| v.color == theme::ACCENT));
    }

    /// Criterion 63: each selected object has its own dashed box.
    #[test]
    fn each_selected_object_draws_its_own_dashed_box() {
        let view = view_at(1.0);
        let a = px_box(0.5, 0.5, 60.0, 40.0, 1.0);
        let b = px_box(200.5, 100.5, 80.0, 50.0, 1.0);
        let one = build_lines(view, &selected_input(a)).triangle_count();
        let other = build_lines(view, &selected_input(b)).triangle_count();
        let both = build_lines(
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
            &build_lines(
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
                let triangles =
                    screen_triangles(&build_lines(view, &selected_input(corners)), view);
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
        let triangles = screen_triangles(&build_lines(view, &selected_input(corners)), view);
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
        assert_eq!(
            build_lines(view, &input).triangles,
            build_lines(view, &input).triangles
        );
    }

    /// Criterion 67: the dashes knock nothing out; every triangle is the
    /// accent colour and nothing is drawn in the gaps.
    #[test]
    fn the_dashes_leave_the_gaps_empty() {
        let view = view_at(1.0);
        let triangles = screen_triangles(
            &build_lines(view, &selected_input(px_box(10.5, 10.5, 100.0, 60.0, 1.0))),
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
                    let list = build_lines(view, &input);
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
            let list = build_lines(view, &input);
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
            build_lines(
                view,
                &SelectDecorationInput {
                    hovered: Some((fixture_id(), corners)),
                    device_pixel_ratio: ratio,
                    ..SelectDecorationInput::default()
                },
            )
        };
        assert_eq!(at(1.0).triangles, at(2.0).triangles);
        let reference = quad_outline(corners, 1.0, theme::HOVER_BOX);
        assert_eq!(at(1.0).triangles, reference.triangles);
    }

    /// A box that is axis-aligned only because it was turned a quarter turn is
    /// snapped like any other.
    #[test]
    fn a_quarter_turned_box_counts_as_axis_aligned() {
        let view = view_at(1.0);
        let corners = turned_box(100.0, 20.3, 50.0, 30.0, 90.0, 1.0);
        let hover = build_lines(view, &hovered_input(corners));
        assert_ne!(
            hover.triangles,
            quad_outline(corners, 1.0, theme::HOVER_BOX).triangles
        );
    }

    /// A ratio that is not a positive finite number reads as 1.
    #[test]
    fn a_nonsense_ratio_reads_as_one() {
        let view = view_at(1.0);
        let corners = px_box(10.3, 20.7, 50.0, 30.0, 1.0);
        let at = |ratio: f64| {
            build_lines(
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
        let list = build_lines(view, &selected_input(px_box(0.5, 0.5, 100_000.0, 8.0, 1.0)));
        assert_eq!(list.triangle_count(), 8);
    }

    // ---- the skew guide replaces the box's dashes on the fixed edge (68) ----

    fn with_guide(
        corners: SelectionBox,
        guide: (Point, Point),
        ratio: f64,
    ) -> SelectDecorationInput {
        SelectDecorationInput {
            selected: vec![(fixture_id(), corners)],
            skew_guide: Some(guide),
            device_pixel_ratio: ratio,
            ..SelectDecorationInput::default()
        }
    }

    /// Samples along the edge `a` to `b` (screen pixels, away from the corners):
    /// inside `(from, to)` along the edge the guide covers it, so the box draws
    /// nothing there; outside, the box is drawn as without the guide.
    fn assert_cut(
        view: ViewTransform,
        corners: SelectionBox,
        edge: usize,
        guide: (Point, Point),
        cut: (f64, f64),
    ) {
        let screen = corners.map(|c| view.document_to_screen(c));
        let (a, b) = (screen[edge], screen[(edge + 1) % 4]);
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        let plain = screen_triangles(&build_lines(view, &selected_input(corners)), view);
        let cut_list = screen_triangles(&build_lines(view, &with_guide(corners, guide, 1.0)), view);
        let mut inside_seen = false;
        let mut t = 1.0;
        while t < length - 1.0 {
            let p = lerp(a, b, t / length);
            if t > cut.0 && t < cut.1 {
                assert!(!covered(&cut_list, p), "t {t} is under the guide");
                inside_seen |= covered(&plain, p);
            } else {
                assert_eq!(
                    covered(&cut_list, p),
                    covered(&plain, p),
                    "t {t} is outside the guide"
                );
            }
            t += 0.25;
        }
        assert!(inside_seen, "the plain box has dashes under the guide");
    }

    #[test]
    fn the_box_edge_on_the_skew_guide_is_not_drawn_where_the_guide_covers_it() {
        let view = view_at(1.0);
        let corners = px_box(10.5, 10.5, 100.0, 60.0, 1.0);
        // The fixed (bottom) edge, the guide reaching 16 px past both ends.
        let guide = (Point::new(-5.5, 70.5), Point::new(126.5, 70.5));
        let plain = build_lines(view, &selected_input(corners)).triangle_count();
        let cut = build_lines(view, &with_guide(corners, guide, 1.0)).triangle_count();
        // The 100 px bottom edge is 15 dashes of two triangles.
        assert_eq!(plain - cut, 30);
        assert_cut(view, corners, 2, guide, (0.0, 100.0 + 32.0));
    }

    #[test]
    fn a_guide_shorter_than_the_edge_cuts_only_its_own_part() {
        let view = view_at(1.0);
        let corners = px_box(10.5, 10.5, 100.0, 60.0, 1.0);
        let guide = (Point::new(40.5, 70.5), Point::new(80.5, 70.5));
        // Edge 2 runs right to left: along-distance from (110.5, 70.5).
        assert_cut(view, corners, 2, guide, (30.0, 70.0));
    }

    #[test]
    fn a_rotated_boxs_fixed_edge_is_cut_too() {
        let view = view_at(1.0);
        let corners = turned_box(80.0, 60.0, 120.0, 70.0, 25.0, 1.0);
        let (a, b) = (corners[1], corners[2]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length = dx.hypot(dy);
        let (ux, uy) = (dx / length, dy / length);
        let guide = (
            Point::new(a.x - ux * 16.0, a.y - uy * 16.0),
            Point::new(b.x + ux * 16.0, b.y + uy * 16.0),
        );
        assert_cut(view, corners, 1, guide, (-16.0, length + 16.0));
    }

    #[test]
    fn a_guide_through_the_centre_cuts_nothing() {
        let view = view_at(1.0);
        let corners = px_box(10.5, 10.5, 100.0, 60.0, 1.0);
        let centre = (Point::new(-5.5, 40.5), Point::new(126.5, 40.5));
        assert_eq!(
            build_lines(view, &with_guide(corners, centre, 1.0)).triangles,
            build_lines(view, &selected_input(corners)).triangles
        );
    }

    #[test]
    fn the_cut_follows_a_snapped_edge_within_a_pixel() {
        // The guide runs on the true edge (y 70.7); the box edge snapped to 70.5.
        let view = view_at(1.0);
        let corners = px_box(10.5, 10.7, 100.0, 60.0, 1.0);
        let guide = (Point::new(-5.5, 70.7), Point::new(126.5, 70.7));
        let plain = build_lines(view, &selected_input(corners)).triangle_count();
        let cut = build_lines(view, &with_guide(corners, guide, 1.0)).triangle_count();
        assert_eq!(plain - cut, 30);
    }

    // ---- the guide's own snap ------------------------------------------

    #[test]
    fn an_axis_aligned_guide_snaps_like_the_box_and_a_rotated_one_does_not() {
        for ratio in [1.0, 1.5, 2.0, 3.0] {
            let view = view_at(1.0);
            let (from, to, width) =
                snap_guide_line(view, Point::new(-5.3, 40.7), Point::new(126.2, 40.7), ratio);
            let w = device_line_width(ratio);
            assert!((width * ratio - w).abs() < 1e-9, "ratio {ratio}");
            // The line is the box's own snapped coordinate.
            assert!((from.y - snap_to_device(40.7, ratio, w)).abs() < 1e-9);
            assert!((to.y - from.y).abs() < 1e-9);
            // The ends are on the device grid.
            for x in [from.x, to.x] {
                assert!(
                    (x * ratio - (x * ratio).round()).abs() < 1e-6,
                    "ratio {ratio}: {x}"
                );
            }
            // A vertical guide likewise.
            let (from, to, _) =
                snap_guide_line(view, Point::new(40.7, -5.3), Point::new(40.7, 126.2), ratio);
            assert!((from.x - snap_to_device(40.7, ratio, w)).abs() < 1e-9);
            assert!((to.x - from.x).abs() < 1e-9);
        }
        let (from, to) = (Point::new(3.3, 4.4), Point::new(90.1, 52.7));
        let rotated = snap_guide_line(view_at(2.0), from, to, 2.0);
        assert_eq!(rotated, (from, to, 1.0));
    }

    // -----------------------------------------------------------------
    // `0007` criteria 40 and 41: casing under the lines, a stronger hover box
    // -----------------------------------------------------------------

    fn dashes_and_casings(list: &DrawList, line: RgbaColor, casing: RgbaColor) -> (usize, usize) {
        let count = |colour: RgbaColor| list.triangles.iter().filter(|v| v.color == colour).count();
        (count(line), count(casing))
    }

    /// The selection box's dashes sit on a casing of the same dashes, three
    /// times as wide across, and every casing triangle comes before every dash
    /// triangle, so a casing never covers a neighbouring dash.
    #[test]
    fn the_selected_boxs_dashes_sit_on_a_three_times_wider_casing_drawn_first() {
        let input = SelectDecorationInput {
            selected: vec![(fixture_id(), axis_box(0.0, 0.0, 100.0, 60.0))],
            ..SelectDecorationInput::default()
        };
        let list = build(ViewTransform::identity(), &input);
        let (dashes, casings) = dashes_and_casings(&list, theme::ACCENT, theme::SELECTION_CASING);
        assert!(dashes > 0);
        assert_eq!(dashes, casings, "one casing quad per dash");
        let last_casing = list
            .triangles
            .iter()
            .rposition(|v| v.color == theme::SELECTION_CASING)
            .unwrap();
        let first_dash = list
            .triangles
            .iter()
            .position(|v| v.color == theme::ACCENT)
            .unwrap();
        assert!(last_casing < first_dash);
        // Across the top edge: 1 px line, 3 px casing.
        let thickness = |colour: RgbaColor| {
            let ys: Vec<f64> = list
                .triangles
                .iter()
                .filter(|v| v.color == colour && v.position.x > 20.0 && v.position.x < 80.0)
                .filter(|v| v.position.y < 30.0)
                .map(|v| v.position.y)
                .collect();
            ys.iter().copied().fold(f64::MIN, f64::max)
                - ys.iter().copied().fold(f64::MAX, f64::min)
        };
        assert!((thickness(theme::ACCENT) - 1.0).abs() < 1e-9);
        assert!((thickness(theme::SELECTION_CASING) - 3.0).abs() < 1e-9);
    }

    /// Criterion 41: the hover box is `--accent` at 65% on a white casing at
    /// 65%, solid; the casing is three times as wide and comes first.
    #[test]
    fn the_hover_box_is_sixty_five_percent_accent_on_a_sixty_five_percent_white_casing() {
        let input = SelectDecorationInput {
            hovered: Some((fixture_id(), axis_box(0.0, 0.0, 100.0, 60.0))),
            ..SelectDecorationInput::default()
        };
        let list = build(ViewTransform::identity(), &input);
        let colours: std::collections::BTreeSet<[u8; 4]> = list
            .triangles
            .iter()
            .map(|v| [v.color.r, v.color.g, v.color.b, v.color.a])
            .collect();
        assert_eq!(
            colours,
            [
                [255, 255, 255, 166],
                [theme::ACCENT.r, theme::ACCENT.g, theme::ACCENT.b, 166]
            ]
            .into_iter()
            .collect()
        );
        let first_line = list
            .triangles
            .iter()
            .position(|v| v.color == theme::HOVER_BOX)
            .unwrap();
        let last_casing = list
            .triangles
            .iter()
            .rposition(|v| v.color == theme::HOVER_BOX_CASING)
            .unwrap();
        assert!(last_casing < first_line);
    }

    // ---- contrast, measured on the composited colours ----

    type Rgb = [f64; 3];

    fn lum(c: Rgb) -> f64 {
        let lin = |v: f64| {
            let v = v / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    }

    fn contrast(a: Rgb, b: Rgb) -> f64 {
        let (la, lb) = (lum(a), lum(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    fn over(top: RgbaColor, below: Rgb) -> Rgb {
        let alpha = f64::from(top.a) / 255.0;
        [
            f64::from(top.r) * alpha + below[0] * (1.0 - alpha),
            f64::from(top.g) * alpha + below[1] * (1.0 - alpha),
            f64::from(top.b) * alpha + below[2] * (1.0 - alpha),
        ]
    }

    const FILLS: [(&str, Rgb); 7] = [
        ("black", [0.0, 0.0, 0.0]),
        ("white", [255.0, 255.0, 255.0]),
        ("accent", [47.0, 111.0, 238.0]),
        ("grey", [128.0, 128.0, 128.0]),
        ("red", [255.0, 0.0, 0.0]),
        ("yellow", [255.0, 220.0, 0.0]),
        ("canvas", [232.0, 232.0, 235.0]),
    ];

    /// Criterion 40: over each fill the better of line and casing is at
    /// least 3:1 for the selection box (and the preview outline, which draws
    /// the same accent over the same casing).
    #[test]
    fn the_selection_box_is_at_least_three_to_one_over_every_fill() {
        for (name, fill) in FILLS {
            let line = contrast(over(theme::ACCENT, fill), fill);
            let casing = contrast(over(theme::SELECTION_CASING, fill), fill);
            assert!(
                line.max(casing) >= 3.0,
                "{name}: line {line:.2}, casing {casing:.2}"
            );
        }
    }

    /// Criterion 40, hover box: the line (65% accent over the 65% casing) and
    /// the casing (65% white) over each fill; the better of the two is at
    /// least 2:1 on all of them (analytic: yellow 2.04, red 2.14, canvas 2.16; read from the GL buffer the weakest, yellow, measured 1.97).
    #[test]
    fn the_hover_box_is_at_least_two_to_one_over_every_fill() {
        for (name, fill) in FILLS {
            let casing_px = over(theme::HOVER_BOX_CASING, fill);
            let line_px = over(theme::HOVER_BOX, casing_px);
            let best = contrast(line_px, fill).max(contrast(casing_px, fill));
            assert!(best >= 2.0, "{name}: {best:.2}");
        }
    }
}
