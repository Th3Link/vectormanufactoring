//! Draws the marquee box and the lasso line while a selection drag runs
//! (`specs/0014-advanced-selection/specification.md`, "UX notes"; the marquee rows
//! of `docs/design-system.md`): a box with a 12 % fill and a solid 1.5 px
//! border in `--marquee-touch` (green) or `--marquee-contain` (red), and a
//! dashed 4 / 3 green line for the lasso. Both are laid out in screen pixels
//! and converted to document millimetres, like every other decoration.

use curvyo_document_core::{Point, Vec2, ViewTransform};

use crate::color::RgbaColor;
use crate::glyphs::{DrawList, circle, thick_line};
use crate::select_box::{effective_ratio, snap_to_device};
use crate::theme;

/// What the Select tool drags out this frame. Mirrors `curvyo-ui-core`'s
/// gesture shape, so this crate depends on no `curvyo-ui-core` type
/// (ADR 0011 §3).
#[derive(Debug, Clone, PartialEq)]
pub enum MarqueeOverlay {
    /// The marquee rectangle between two opposite corners.
    Box {
        /// The press point.
        from: Point,
        /// The pointer.
        to: Point,
        /// Contain mode (red), else touch mode (green).
        contain: bool,
    },
    /// The lasso's freehand line, in document space.
    Lasso(Vec<Point>),
}

/// The overlay as a draw list, to be drawn above everything else.
/// `device_pixel_ratio` is `window.devicePixelRatio`: the box's border is laid
/// on whole device pixels and a whole number of them wide, so it is crisp at
/// any ratio (a value that is not a positive finite number reads as 1).
#[must_use]
pub fn build_marquee_overlay(
    view: ViewTransform,
    overlay: &MarqueeOverlay,
    device_pixel_ratio: f64,
) -> DrawList {
    match overlay {
        MarqueeOverlay::Box { from, to, contain } => marquee_box(
            view,
            *from,
            *to,
            *contain,
            effective_ratio(device_pixel_ratio),
        ),
        MarqueeOverlay::Lasso(points) => lasso_line(view, points),
    }
}

fn marquee_box(view: ViewTransform, from: Point, to: Point, contain: bool, ratio: f64) -> DrawList {
    let mut list = DrawList::default();
    let color = if contain {
        theme::MARQUEE_CONTAIN
    } else {
        theme::MARQUEE_TOUCH
    };
    // Screen pixels, snapped so both edges of the border fall on device pixel
    // boundaries.
    let device_width = (theme::MARQUEE_STROKE_PX * ratio).round().max(1.0);
    let (a, b) = (view.document_to_screen(from), view.document_to_screen(to));
    let snap = |v: f64| snap_to_device(v, ratio, device_width);
    let (left, right) = (snap(a.0.min(b.0)), snap(a.0.max(b.0)));
    let (top, bottom) = (snap(a.1.min(b.1)), snap(a.1.max(b.1)));
    let at = |x: f64, y: f64| view.screen_to_document(x, y);
    let width = device_width / ratio / view.scale();
    if right - left <= f64::EPSILON || bottom - top <= f64::EPSILON {
        // A box with no width or height has no area to fill; its border is
        // the line itself.
        list.extend(thick_line(at(left, top), at(right, bottom), width, color));
        return list;
    }
    let corners = [
        at(left, top),
        at(right, top),
        at(right, bottom),
        at(left, bottom),
    ];
    list.extend(fill_quad(
        corners,
        color.with_alpha(theme::MARQUEE_FILL_ALPHA),
    ));
    // Each border line reaches half a width past its corners, so the corners
    // close.
    let half = width / 2.0;
    for (a, b, along) in [
        (corners[0], corners[1], Vec2::new(half, 0.0)),
        (corners[1], corners[2], Vec2::new(0.0, half)),
        (corners[2], corners[3], Vec2::new(-half, 0.0)),
        (corners[3], corners[0], Vec2::new(0.0, -half)),
    ] {
        list.extend(thick_line(
            a.translated(along.negated()),
            b.translated(along),
            width,
            color,
        ));
    }
    list
}

/// A filled quad of the four `corners`, as two triangles.
fn fill_quad(corners: [Point; 4], color: RgbaColor) -> DrawList {
    let mut list = DrawList::default();
    for index in [0, 1, 2, 0, 2, 3] {
        list.push_vertex(crate::glyphs::Vertex {
            position: corners[index],
            color,
        });
    }
    list
}

/// The polyline as dashes of 4 px with 3 px between, starting with a dash at
/// the first point, the pattern running on across the vertices. A dash that
/// bends at a vertex is drawn as its straight pieces joined by a round dot.
fn lasso_line(view: ViewTransform, points: &[Point]) -> DrawList {
    let mut list = DrawList::default();
    let scale = view.scale();
    let width = theme::MARQUEE_STROKE_PX / scale;
    let (dash, gap) = (theme::LASSO_DASH_PX / scale, theme::LASSO_GAP_PX / scale);
    let color = theme::MARQUEE_TOUCH;
    let on_screen_px: f64 = points
        .windows(2)
        .map(|pair| pair[0].vector_to(pair[1]).length() * scale)
        .sum();
    if on_screen_px.is_nan() || on_screen_px > theme::LASSO_MAX_DASHED_PX {
        // Past the cap the line is solid: a dash count in the billions would
        // hang the frame. A segment that is not finite draws nothing.
        for pair in points.windows(2) {
            if pair[0].vector_to(pair[1]).length().is_finite() {
                list.extend(thick_line(pair[0], pair[1], width, color));
            }
        }
        return list;
    }
    // Where we are in the pattern: drawing a dash, with `remaining` of it still to
    // go, or skipping a gap.
    let mut drawing = true;
    let mut remaining = dash;
    // The start of the dash piece being drawn, if the dash is open.
    let mut piece_start: Option<Point> = points.first().copied();
    for (index, pair) in points.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let final_pair = index + 2 == points.len();
        let length = a.vector_to(b).length();
        if length <= f64::EPSILON {
            continue;
        }
        let mut travelled = 0.0;
        while travelled < length {
            let step = (length - travelled).min(remaining);
            travelled += step;
            remaining -= step;
            let at = lerp(a, b, travelled / length);
            if remaining <= f64::EPSILON {
                if drawing {
                    if let Some(start) = piece_start.take() {
                        list.extend(thick_line(start, at, width, color));
                    }
                    remaining = gap;
                } else {
                    piece_start = Some(at);
                    remaining = dash;
                }
                drawing = !drawing;
            }
        }
        // The segment ended inside a dash: close the piece at the vertex and
        // continue it from there, with a dot over the joint (none at the
        // line's end).
        if drawing && let Some(start) = piece_start.replace(b) {
            list.extend(thick_line(start, b, width, color));
            if !final_pair {
                list.extend(circle(b, width, color));
            }
        }
    }
    list
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors(list: &DrawList) -> Vec<RgbaColor> {
        let mut seen: Vec<RgbaColor> = Vec::new();
        for vertex in &list.triangles {
            if !seen.contains(&vertex.color) {
                seen.push(vertex.color);
            }
        }
        seen
    }

    /// The box is a fill plus four border lines: 2 + 4 * 2 triangles, the fill
    /// at 12 % of the mode colour, the border solid.
    #[test]
    fn the_box_is_a_translucent_fill_with_a_solid_border() {
        for (contain, color) in [
            (false, theme::MARQUEE_TOUCH),
            (true, theme::MARQUEE_CONTAIN),
        ] {
            let list = build_marquee_overlay(
                ViewTransform::identity(),
                &MarqueeOverlay::Box {
                    from: Point::new(10.0, 40.0),
                    to: Point::new(50.0, 10.0),
                    contain,
                },
                1.0,
            );
            assert_eq!(list.triangle_count(), 10);
            assert_eq!(
                colors(&list),
                vec![color.with_alpha(theme::MARQUEE_FILL_ALPHA), color],
                "fill first, then the opaque border"
            );
        }
    }

    /// The border is a whole number of device pixels wide (`round(1.5 * ratio)`:
    /// 2 px at ratio 1, 1.5 px at ratio 2), at any zoom, with both its edges on
    /// device pixel boundaries and its corners closed.
    #[test]
    fn the_border_is_whole_device_pixels_whatever_the_zoom() {
        for ratio in [1.0, 2.0] {
            let want_px = (1.5_f64 * ratio).round() / ratio;
            for scale in [0.5, 1.0, 4.0] {
                let view = ViewTransform::new(scale, Point::new(0.0, 0.0));
                let list = build_marquee_overlay(
                    view,
                    &MarqueeOverlay::Box {
                        from: Point::new(0.0, 0.0),
                        to: Point::new(100.0, 60.0),
                        contain: false,
                    },
                    ratio,
                );
                let border: Vec<Point> = list
                    .triangles
                    .iter()
                    .filter(|v| v.color == theme::MARQUEE_TOUCH)
                    .map(|v| v.position)
                    .collect();
                let top: Vec<f64> = border.iter().take(6).map(|p| p.y * scale).collect();
                let (low, high) = top
                    .iter()
                    .fold((f64::MAX, f64::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
                assert!(
                    (high - low - want_px).abs() < 1e-9,
                    "ratio {ratio} scale {scale}"
                );
                let on_grid = |v: f64| ((v * ratio).round() - v * ratio).abs() < 1e-6;
                assert!(on_grid(low) && on_grid(high), "edges on device pixels");
                let min_x = border.iter().map(|p| p.x).fold(f64::MAX, f64::min) * scale;
                let max_x = border.iter().map(|p| p.x).fold(f64::MIN, f64::max) * scale;
                assert!(on_grid(min_x) && on_grid(max_x), "corners on device pixels");
                assert!(
                    min_x < 0.0 && max_x > 100.0 * scale,
                    "corners close past the box"
                );
            }
        }
    }

    #[test]
    fn a_box_with_no_area_draws_only_its_line() {
        let list = build_marquee_overlay(
            ViewTransform::identity(),
            &MarqueeOverlay::Box {
                from: Point::new(0.0, 5.0),
                to: Point::new(30.0, 5.0),
                contain: true,
            },
            1.0,
        );
        assert_eq!(list.triangle_count(), 2);
        assert_eq!(colors(&list), vec![theme::MARQUEE_CONTAIN]);
    }

    /// 4 px on, 3 px off from the first point: a straight 100 px line has 15
    /// dashes, the second from 7 to 11, the last cut at 100.
    #[test]
    fn the_lasso_is_dashed_four_on_three_off() {
        let list = build_marquee_overlay(
            ViewTransform::identity(),
            &MarqueeOverlay::Lasso(vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)]),
            1.0,
        );
        assert_eq!(list.triangle_count(), 2 * 15);
        assert_eq!(colors(&list), vec![theme::MARQUEE_TOUCH], "always green");
        let spans: Vec<(f64, f64)> = list
            .triangles
            .chunks(6)
            .map(|dash| {
                let xs = dash.iter().map(|v| v.position.x);
                (
                    xs.clone().fold(f64::MAX, f64::min),
                    xs.fold(f64::MIN, f64::max),
                )
            })
            .collect();
        assert!((spans[0].0 - 0.0).abs() < 1e-9 && (spans[0].1 - 4.0).abs() < 1e-9);
        assert!((spans[1].0 - 7.0).abs() < 1e-9 && (spans[1].1 - 11.0).abs() < 1e-9);
        assert!((spans[14].0 - 98.0).abs() < 1e-9 && (spans[14].1 - 100.0).abs() < 1e-9);
    }

    /// The pattern runs on across a vertex: two 50 px stretches make the same
    /// dashes as one 100 px line, and a dash that bends is joined by a dot.
    #[test]
    fn the_dash_pattern_continues_across_vertices() {
        let straight = build_marquee_overlay(
            ViewTransform::identity(),
            &MarqueeOverlay::Lasso(vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)]),
            1.0,
        );
        let bent = build_marquee_overlay(
            ViewTransform::identity(),
            &MarqueeOverlay::Lasso(vec![
                Point::new(0.0, 0.0),
                Point::new(50.0, 0.0),
                Point::new(100.0, 0.0),
            ]),
            1.0,
        );
        let covered = |list: &DrawList, x: f64| {
            list.triangles.chunks(3).any(|t| {
                let (lo, hi) = t.iter().fold((f64::MAX, f64::MIN), |(l, h), v| {
                    (l.min(v.position.x), h.max(v.position.x))
                });
                lo <= x && x <= hi && hi - lo > 0.5
            })
        };
        for x in (0..100).map(|i| f64::from(i) + 0.5) {
            assert_eq!(covered(&straight, x), covered(&bent, x), "x = {x}");
        }
    }

    /// Past the cap the line is solid: a 1e9 mm segment is one quad, not a
    /// billion dashes.
    #[test]
    fn a_line_past_the_cap_is_solid_and_bounded() {
        let list = build_marquee_overlay(
            ViewTransform::identity(),
            &MarqueeOverlay::Lasso(vec![Point::new(0.0, 0.0), Point::new(1e9, 0.0)]),
            1.0,
        );
        assert_eq!(list.triangle_count(), 2);
        let near_cap = build_marquee_overlay(
            ViewTransform::identity(),
            &MarqueeOverlay::Lasso(vec![Point::new(0.0, 0.0), Point::new(49_000.0, 0.0)]),
            1.0,
        );
        assert!(
            near_cap.triangle_count() <= 2 * 7_100,
            "dashed below the cap"
        );
    }

    #[test]
    fn a_line_with_fewer_than_two_points_draws_nothing() {
        for points in [vec![], vec![Point::new(3.0, 3.0)]] {
            let list = build_marquee_overlay(
                ViewTransform::identity(),
                &MarqueeOverlay::Lasso(points),
                1.0,
            );
            assert_eq!(list.triangle_count(), 0);
        }
    }

    /// The dash is 4 screen pixels at any zoom.
    #[test]
    fn the_dash_length_is_in_screen_pixels() {
        let view = ViewTransform::new(2.0, Point::new(0.0, 0.0));
        let list = build_marquee_overlay(
            view,
            &MarqueeOverlay::Lasso(vec![Point::new(0.0, 0.0), Point::new(20.0, 0.0)]),
            1.0,
        );
        let xs: Vec<f64> = list
            .triangles
            .iter()
            .take(6)
            .map(|v| v.position.x)
            .collect();
        let (lo, hi) = xs
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
        assert!(((hi - lo) * 2.0 - 4.0).abs() < 1e-9);
    }
}
