//! The document's own area, painted under the artwork
//! (`specs/0015-document-size-and-rulers/` criterion 28): a flat rectangle in
//! `--canvas-bg` over the pasteboard the GPU clears to.

use curvyo_document_core::{DocumentSize, Point, ViewTransform};

use crate::glyphs::{DrawList, Vertex};
use crate::theme;

/// The colour at a document point: the document's `--canvas-bg` inside the
/// document rectangle, the `--pasteboard-bg` outside it. What a knockout dot
/// drawn there must be filled with to read as a hole.
#[must_use]
pub fn background_at(size: DocumentSize, point: Point) -> crate::RgbaColor {
    let inside = (0.0..=size.width.as_mm()).contains(&point.x)
        && (0.0..=size.height.as_mm()).contains(&point.y);
    if inside {
        theme::CANVAS_BG
    } else {
        theme::PASTEBOARD_BG
    }
}

/// Builds the document rectangle as the bottom artwork layer. Its edges are
/// snapped to whole device pixels (`device_pixel_ratio` device pixels per CSS
/// pixel) so the edge is one colour step and never two half-tone rows.
#[must_use]
pub fn build_document_area(
    size: DocumentSize,
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

    let mut list = DrawList::default();
    for position in [
        top_left,
        top_right,
        bottom_right,
        top_left,
        bottom_right,
        bottom_left,
    ] {
        list.push_vertex(Vertex {
            position,
            color: theme::CANVAS_BG,
        });
    }
    list.close_layer();
    list
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Criterion 28: the area is the document rectangle, one artwork layer of
    /// two triangles in the canvas colour.
    #[test]
    fn the_area_is_the_document_rectangle_in_the_canvas_colour() {
        let view = ViewTransform::new(1.0, Point::new(0.0, 0.0));
        let list = build_document_area(DocumentSize::from_mm(210.0, 297.0), view, 1.0);
        assert_eq!(list.triangle_count(), 2);
        assert_eq!(list.layers(), &[6]);
        assert!(list.triangles.iter().all(|v| v.color == theme::CANVAS_BG));
        let (min, max) = bounds(&list);
        assert_eq!((min, max), (Point::new(0.0, 0.0), Point::new(210.0, 297.0)));
    }

    /// The edges land on whole device pixels at any zoom and pan, also on a
    /// 1.5 device-pixel-ratio display.
    #[test]
    fn the_edges_are_snapped_to_whole_device_pixels() {
        for dpr in [1.0, 1.5, 2.0] {
            let view = ViewTransform::new(3.779_527_559, Point::new(-19.3, -7.77));
            let list = build_document_area(DocumentSize::from_mm(210.0, 297.0), view, dpr);
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

    /// Criterion 31: inside the document a knockout is the canvas colour, on
    /// the pasteboard the pasteboard colour, on the edge the document's.
    #[test]
    fn the_background_follows_the_document_edge() {
        let size = DocumentSize::from_mm(100.0, 50.0);
        assert_eq!(
            background_at(size, Point::new(10.0, 10.0)),
            theme::CANVAS_BG
        );
        assert_eq!(background_at(size, Point::new(0.0, 50.0)), theme::CANVAS_BG);
        assert_eq!(
            background_at(size, Point::new(-0.1, 10.0)),
            theme::PASTEBOARD_BG
        );
        assert_eq!(
            background_at(size, Point::new(10.0, 50.1)),
            theme::PASTEBOARD_BG
        );
        assert_eq!(
            background_at(size, Point::new(500.0, 500.0)),
            theme::PASTEBOARD_BG
        );
    }
}
