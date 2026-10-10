//! The document's own area, painted under the artwork
//! (`specs/0015-document-size-and-rulers/` criterion 28,
//! `specs/0040-document-background`): the document background over the
//! pasteboard the GPU clears to. An opaque colour is one quad; None or a
//! translucent colour first gets a checkerboard quad that the host paints with
//! its checkerboard shader ([`CheckerGrid`] says where the cells are).

use curvyo_document_core::{
    BackgroundPaint, DocumentBackground, DocumentSize, Point, ViewTransform,
};

use crate::artwork::paint;
use crate::color::RgbaColor;
use crate::glyphs::{DrawList, Vertex};
use crate::theme;

/// The side of a checkerboard cell, CSS pixels (`docs/design-system.md`, row
/// "Canvas checkerboard"). Fixed on screen: it does not change with the zoom.
const CHECKER_CELL_CSS_PX: f64 = 8.0;

/// Where the document area's checkerboard cells are, in device pixels of the
/// canvas (origin at its top-left): anchored at the snapped document corner, cells
/// of `round(8 x devicePixelRatio)` device pixels (at least 1), the cell at the
/// corner in [`theme::CHECKER_A`] and the tones alternating like a chessboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckerGrid {
    /// The device-pixel column of the document's left edge.
    pub corner_x: i64,
    /// The device-pixel row of the document's top edge.
    pub corner_y: i64,
    /// The side of a cell, device pixels.
    pub cell: u32,
}

impl CheckerGrid {
    /// The corner reduced to the pattern's period, two cells, so a host can pass
    /// it to a shader as a small exact number however far the page is panned.
    #[must_use]
    pub fn phase(self) -> (u32, u32) {
        let period = i64::from(self.cell) * 2;
        // The remainders are in 0..period, which fits a `u32`.
        let reduce = |corner: i64| u32::try_from(corner.rem_euclid(period)).unwrap_or(0);
        (reduce(self.corner_x), reduce(self.corner_y))
    }
}

/// The checkerboard grid of a frame: the document corner snapped to a whole
/// device pixel exactly as the document area's edge is, and the cell size.
#[must_use]
pub fn checker_grid(view: ViewTransform, device_pixel_ratio: f64) -> CheckerGrid {
    let (x, y) = view.document_to_screen(Point::new(0.0, 0.0));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (corner_x, corner_y, cell) = (
        (x * device_pixel_ratio).round() as i64,
        (y * device_pixel_ratio).round() as i64,
        (CHECKER_CELL_CSS_PX * device_pixel_ratio).round().max(1.0) as u32,
    );
    CheckerGrid {
        corner_x,
        corner_y,
        cell,
    }
}

/// The tone of the checkerboard at device pixel `(x, y)`: the parity of
/// `floor((p - corner) / cell)` summed over both axes, even meaning
/// [`theme::CHECKER_A`]. The fragment shader computes the same one-liner.
#[must_use]
pub fn checker_tone(grid: CheckerGrid, x: i64, y: i64) -> RgbaColor {
    let cell = i64::from(grid.cell);
    let column = (x - grid.corner_x).div_euclid(cell);
    let row = (y - grid.corner_y).div_euclid(cell);
    if (column + row).rem_euclid(2) == 0 {
        theme::CHECKER_A
    } else {
        theme::CHECKER_B
    }
}

/// `top` laid over `base`, both opaque results: the colour a translucent `top`
/// shows on an opaque `base`.
fn composite(top: RgbaColor, base: RgbaColor) -> RgbaColor {
    let alpha = f64::from(top.a) / 255.0;
    let mix = |t: u8, b: u8| {
        let value = f64::from(t).mul_add(alpha, f64::from(b) * (1.0 - alpha));
        // `value` is between the two channels, so it fits a byte.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let byte = value.round() as u8;
        byte
    };
    RgbaColor::opaque(mix(top.r, base.r), mix(top.g, base.g), mix(top.b, base.b))
}

/// The colour at a document point: the document background inside the document
/// rectangle (edges included), the `--pasteboard-bg` outside it. What a knockout
/// dot drawn there must be filled with to read as a hole. A background that is
/// None or translucent shows the checkerboard; its knockout is the colour
/// composited over [`theme::CHECKER_A`] (`0040` criterion 46).
#[must_use]
pub fn background_at(
    size: DocumentSize,
    background: DocumentBackground,
    point: Point,
) -> RgbaColor {
    if !size.contains(point) {
        return theme::PASTEBOARD_BG;
    }
    match background.paint {
        BackgroundPaint::None => theme::CHECKER_A,
        BackgroundPaint::Solid => {
            let colour = paint(background.color, background.opacity);
            if colour.a == 255 {
                colour
            } else {
                composite(colour, theme::CHECKER_A)
            }
        }
    }
}

/// Builds the document rectangle as the bottom artwork layer(s). Its edges are
/// snapped to whole device pixels (`device_pixel_ratio` device pixels per CSS
/// pixel) so the edge is one colour step and never two half-tone rows.
///
/// An opaque colour is one quad. None is one quad that the host paints as a
/// checkerboard ([`DrawList::checker_end`]). A translucent colour is that
/// checkerboard quad and, as the next layer, the colour quad over it.
#[must_use]
pub fn build_document_area(
    size: DocumentSize,
    background: DocumentBackground,
    view: ViewTransform,
    device_pixel_ratio: f64,
) -> DrawList {
    let snap = |css_px: f64| (css_px * device_pixel_ratio).round() / device_pixel_ratio;
    let corner = |x_mm: f64, y_mm: f64| {
        let (x, y) = view.document_to_screen(Point::new(x_mm, y_mm));
        view.screen_to_document(snap(x), snap(y))
    };
    let top_left = corner(0.0, 0.0);
    let bottom_right = corner(size.width.as_mm(), size.height.as_mm());
    let top_right = Point::new(bottom_right.x, top_left.y);
    let bottom_left = Point::new(top_left.x, bottom_right.y);
    let corners = [
        top_left,
        top_right,
        bottom_right,
        top_left,
        bottom_right,
        bottom_left,
    ];
    let quad = |list: &mut DrawList, color: RgbaColor| {
        for position in corners {
            list.push_vertex(Vertex { position, color });
        }
    };

    let colour = paint(background.color, background.opacity);
    let mut list = DrawList::default();
    match background.paint {
        BackgroundPaint::Solid if colour.a == 255 => {
            quad(&mut list, colour);
            list.close_layer();
        }
        BackgroundPaint::None => {
            quad(&mut list, theme::CHECKER_A);
            list.close_checker_prefix();
        }
        BackgroundPaint::Solid => {
            quad(&mut list, theme::CHECKER_A);
            list.close_checker_prefix();
            quad(&mut list, colour);
            list.close_layer();
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{Color, Opacity};

    use super::*;

    fn view(scale: f64, x: f64, y: f64) -> ViewTransform {
        ViewTransform::new(scale, Point::new(x, y))
    }

    fn solid(r: u8, g: u8, b: u8, alpha: f64) -> DocumentBackground {
        DocumentBackground {
            paint: BackgroundPaint::Solid,
            color: Color { r, g, b },
            opacity: Opacity::new(alpha).unwrap(),
        }
    }

    fn none() -> DocumentBackground {
        DocumentBackground {
            paint: BackgroundPaint::None,
            ..DocumentBackground::DEFAULT
        }
    }

    fn area(background: DocumentBackground) -> DrawList {
        let identity = view(1.0, 0.0, 0.0);
        build_document_area(
            DocumentSize::from_mm(210.0, 297.0),
            background,
            identity,
            1.0,
        )
    }

    fn bounds(list: &DrawList) -> (Point, Point) {
        let xs = list.triangles.iter().map(|v| v.position.x);
        let ys = list.triangles.iter().map(|v| v.position.y);
        (
            Point::new(
                xs.clone().fold(f64::INFINITY, f64::min),
                ys.clone().fold(f64::INFINITY, f64::min),
            ),
            Point::new(
                xs.fold(f64::NEG_INFINITY, f64::max),
                ys.fold(f64::NEG_INFINITY, f64::max),
            ),
        )
    }

    /// Criterion 9 of `0040`, and 28 of `0015`: the default is the document
    /// rectangle, one artwork layer of two triangles in the canvas colour, with no
    /// checkerboard, exactly the list it always was.
    #[test]
    fn the_default_is_one_quad_in_the_canvas_colour() {
        let list = area(DocumentBackground::DEFAULT);
        assert_eq!(list.triangle_count(), 2);
        assert_eq!(list.layers(), &[6]);
        assert_eq!(list.checker_end(), 0);
        assert!(list.triangles.iter().all(|v| v.color == theme::CANVAS_BG));
        let (min, max) = bounds(&list);
        assert_eq!((min, max), (Point::new(0.0, 0.0), Point::new(210.0, 297.0)));
    }

    /// Criterion 10: an opaque colour is painted as given, still one quad.
    #[test]
    fn an_opaque_colour_is_one_quad_in_that_colour() {
        let list = area(solid(255, 0, 0, 1.0));
        assert_eq!((list.layers(), list.checker_end()), (&[6][..], 0));
        assert!(
            list.triangles
                .iter()
                .all(|v| v.color == RgbaColor::opaque(255, 0, 0))
        );
    }

    /// Criterion 12 and 47: None is one checkerboard quad, as many layers as an
    /// opaque colour has, plus the prefix.
    #[test]
    fn none_is_one_checkerboard_quad() {
        let list = area(none());
        assert_eq!(list.layers(), &[6]);
        assert_eq!(list.checker_end(), 6);
        assert_eq!(
            list.layers().len(),
            area(solid(1, 2, 3, 1.0)).layers().len()
        );
        let (min, max) = bounds(&list);
        assert_eq!((min, max), (Point::new(0.0, 0.0), Point::new(210.0, 297.0)));
    }

    /// Criteria 11 and 47: a translucent colour is the checkerboard quad and
    /// the colour quad over it, one layer more than None.
    #[test]
    fn a_translucent_colour_is_the_checkerboard_and_a_colour_layer() {
        let list = area(solid(255, 0, 0, 128.0 / 255.0));
        assert_eq!(list.layers(), &[6, 12]);
        assert_eq!(list.checker_end(), 6);
        assert!(
            list.triangles[6..]
                .iter()
                .all(|v| v.color == RgbaColor::opaque(255, 0, 0).with_alpha(128))
        );
        // A solid colour at alpha 0 is still a layer over the checkerboard.
        assert_eq!(area(solid(255, 0, 0, 0.0)).layers(), &[6, 12]);
    }

    /// The count of criterion 47: the draw command count does not depend on the
    /// size of the document or the zoom.
    #[test]
    fn the_list_does_not_grow_with_zoom_or_size() {
        for (scale, mm) in [(0.02, 100_000.0), (1.0, 210.0), (80.0, 1.0)] {
            let list = build_document_area(
                DocumentSize::from_mm(mm, mm),
                none(),
                view(scale, 0.0, 0.0),
                1.0,
            );
            assert_eq!(list.triangle_count(), 2, "{scale}");
        }
    }

    /// The edges land on whole device pixels at any zoom and pan, also on a
    /// 1.5 device-pixel-ratio display, for every kind of background.
    #[test]
    fn the_edges_are_snapped_to_whole_device_pixels() {
        for background in [DocumentBackground::DEFAULT, none(), solid(1, 2, 3, 0.5)] {
            for dpr in [1.0, 1.5, 2.0] {
                let view = view(3.779_527_559, -19.3, -7.77);
                let list =
                    build_document_area(DocumentSize::from_mm(210.0, 297.0), background, view, dpr);
                let (min, max) = bounds(&list);
                for point in [min, max] {
                    let (x, y) = view.document_to_screen(point);
                    for css_px in [x, y] {
                        let device = css_px * dpr;
                        assert!((device - device.round()).abs() < 1e-6, "{dpr}: {device}");
                    }
                }
            }
        }
    }

    /// Criterion 12: cells of `round(8 x ratio)` device pixels, at least 1; at a
    /// ratio of 1.1 the cell rounds to 9.
    #[test]
    fn the_cell_is_eight_css_pixels_rounded_to_device_pixels() {
        let cell = |dpr: f64| checker_grid(view(1.0, 0.0, 0.0), dpr).cell;
        assert_eq!(cell(1.0), 8);
        assert_eq!(cell(2.0), 16);
        assert_eq!(cell(1.1), 9);
        assert_eq!(cell(1.5), 12);
        assert_eq!(cell(0.01), 1, "never below one device pixel");
    }

    /// Criterion 12: the cell does not change with the zoom, and the pattern
    /// moves with the page on a pan: the corner is where the page's corner is.
    #[test]
    fn the_cell_is_fixed_on_screen_and_the_pattern_is_anchored_at_the_corner() {
        let at = |scale: f64, x: f64, y: f64| checker_grid(view(scale, x, y), 1.0);
        for scale in [0.02, 1.0, 80.0] {
            assert_eq!(at(scale, 0.0, 0.0).cell, 8, "{scale}");
        }
        // The page's corner is at the document point (0, 0): screen (-x*s, -y*s)
        // for a view whose origin is the document point at screen (0, 0).
        let panned = at(2.0, -30.0, -50.0);
        assert_eq!((panned.corner_x, panned.corner_y), (60, 100));
        // Shifting the page by whole periods leaves the phase alone.
        let a = at(1.0, 0.0, 0.0).phase();
        let b = at(1.0, -16.0, -32.0).phase();
        assert_eq!(a, b);
    }

    /// Criterion 12, the placement test: the cell at the corner is
    /// `--checker-a`; at ratio 1 the pixel 4 right and 4 below is white and the
    /// pixel 12 right and 4 below is `#C9C9CE`; at ratio 2 the same points scale.
    #[test]
    fn the_tones_follow_the_chessboard_from_the_corner() {
        for (dpr, scale) in [(1.0, 1u32), (2.0, 2)] {
            let grid = checker_grid(view(1.0, -10.0, -10.0), dpr);
            let (cx, cy) = (grid.corner_x, grid.corner_y);
            let at = |dx: u32, dy: u32| {
                checker_tone(grid, cx + i64::from(dx * scale), cy + i64::from(dy * scale))
            };
            assert_eq!(at(0, 0), theme::CHECKER_A, "the corner cell");
            assert_eq!(at(4, 4), theme::CHECKER_A, "ratio {dpr}");
            assert_eq!(at(12, 4), theme::CHECKER_B, "ratio {dpr}");
            assert_eq!(at(4, 12), theme::CHECKER_B, "ratio {dpr}");
            assert_eq!(at(12, 12), theme::CHECKER_A, "ratio {dpr}");
            assert_eq!(at(7, 7), theme::CHECKER_A, "the last pixel of the cell");
            assert_eq!(at(8, 7), theme::CHECKER_B, "the first pixel of the next");
        }
    }

    /// The pattern is the same on both sides of the corner (the shader sees
    /// negative offsets for nothing, but the formula must not break).
    #[test]
    fn the_tone_formula_continues_before_the_corner() {
        let grid = CheckerGrid {
            corner_x: 100,
            corner_y: 100,
            cell: 8,
        };
        assert_eq!(checker_tone(grid, 99, 100), theme::CHECKER_B);
        assert_eq!(checker_tone(grid, 99, 99), theme::CHECKER_A);
        assert_eq!(checker_tone(grid, 91, 100), theme::CHECKER_A);
        assert_eq!(checker_tone(grid, 92, 100), theme::CHECKER_B);
        assert_eq!(checker_tone(grid, 90, 100), theme::CHECKER_A);
        let (px, py) = grid.phase();
        assert_eq!((px, py), (100 % 16, 100 % 16));
    }

    /// Criterion 46: inside the document a knockout is the document background,
    /// on the pasteboard the pasteboard colour, on the edge the document's.
    #[test]
    fn the_background_follows_the_document_edge() {
        let size = DocumentSize::from_mm(100.0, 50.0);
        let default = DocumentBackground::DEFAULT;
        assert_eq!(
            background_at(size, default, Point::new(10.0, 10.0)),
            theme::CANVAS_BG
        );
        assert_eq!(
            background_at(size, default, Point::new(0.0, 50.0)),
            theme::CANVAS_BG
        );
        for outside in [(-0.1, 10.0), (10.0, 50.1), (500.0, 500.0)] {
            assert_eq!(
                background_at(size, default, Point::new(outside.0, outside.1)),
                theme::PASTEBOARD_BG,
                "{outside:?}"
            );
        }
    }

    #[test]
    fn the_knockout_over_other_backgrounds() {
        let size = DocumentSize::from_mm(100.0, 50.0);
        let inside = Point::new(10.0, 10.0);
        assert_eq!(
            background_at(size, solid(255, 0, 0, 1.0), inside),
            RgbaColor::opaque(255, 0, 0)
        );
        assert_eq!(background_at(size, none(), inside), theme::CHECKER_A);
        // Criterion 11: `#FF000080` over the white cell is `#FF7F7F`.
        assert_eq!(
            background_at(size, solid(255, 0, 0, 128.0 / 255.0), inside),
            RgbaColor::opaque(255, 127, 127)
        );
        assert_eq!(
            background_at(size, solid(255, 0, 0, 0.0), inside),
            theme::CHECKER_A
        );
        assert_eq!(
            background_at(size, none(), Point::new(-1.0, 0.0)),
            theme::PASTEBOARD_BG
        );
        assert_eq!(
            background_at(size, solid(255, 0, 0, 1.0), Point::new(f64::NAN, 5.0)),
            theme::PASTEBOARD_BG
        );
    }

    /// `extend` keeps the prefix of the area and the layers of the artwork after it.
    #[test]
    fn extending_the_area_keeps_its_checkerboard_prefix() {
        let mut frame = area(none());
        let mut artwork = DrawList::default();
        artwork.push_vertex(Vertex {
            position: Point::new(1.0, 1.0),
            color: RgbaColor::BLACK,
        });
        artwork.push_vertex(Vertex {
            position: Point::new(2.0, 1.0),
            color: RgbaColor::BLACK,
        });
        artwork.push_vertex(Vertex {
            position: Point::new(2.0, 2.0),
            color: RgbaColor::BLACK,
        });
        artwork.close_layer();
        frame.extend(artwork);
        assert_eq!(frame.checker_end(), 6);
        assert_eq!(frame.layers(), &[6, 9]);
    }
}
