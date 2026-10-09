//! The list the host submits each frame: the document's own area under
//! everything `Session::draw_list` builds (`specs/0015-document-size-and-rulers/`
//! criterion 28).

use curvyo_render_core::{DrawList, build_document_area};

use super::Session;

impl Session {
    /// The frame to submit: the document rectangle as the bottom artwork layer
    /// (it pans and zooms with the content, and the GPU's pasteboard clear
    /// shows around it), then [`Session::draw_list`]. Kept apart from
    /// `draw_list` so the area is not mistaken for artwork of the document.
    #[must_use]
    pub fn frame_draw_list(&self) -> DrawList {
        let mut list =
            build_document_area(self.document.size(), self.view(), self.device_pixel_ratio);
        list.extend(self.draw_list());
        list
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::DocumentSize;
    use curvyo_render_core::Vertex;

    use super::*;

    /// The area is layer 0 and the artwork layers follow, with the overlay
    /// after them: nothing of `draw_list` is lost or reordered.
    #[test]
    fn the_document_area_is_the_bottom_layer_under_the_unchanged_draw_list() {
        let session = Session::new(1);
        let frame = session.frame_draw_list();
        let inner = session.draw_list();
        assert_eq!(frame.layers().first(), Some(&6), "two triangles");
        assert_eq!(
            frame.triangles.len(),
            inner.triangles.len() + 6,
            "area plus the unchanged list"
        );
        assert_eq!(frame.triangles[6..], inner.triangles[..]);
        assert_eq!(frame.layers().len(), inner.layers().len() + 1);
        assert_eq!(frame.overlay_start(), inner.overlay_start() + 6);
    }

    /// The area covers the document rectangle: (0, 0) to the document's size,
    /// within the device-pixel snap.
    #[test]
    fn the_area_covers_the_document_rectangle() {
        let mut session = Session::new(1);
        session.show_default_view();
        let frame = session.frame_draw_list();
        let area = &frame.triangles[..6];
        let extent = |value: fn(&Vertex) -> f64, pick: fn(f64, f64) -> f64, start: f64| {
            area.iter().map(value).fold(start, pick)
        };
        let size = DocumentSize::default();
        assert!(extent(|v| v.position.x, f64::min, f64::INFINITY).abs() < 0.3);
        assert!(extent(|v| v.position.y, f64::min, f64::INFINITY).abs() < 0.3);
        let right = extent(|v| v.position.x, f64::max, f64::NEG_INFINITY);
        let bottom = extent(|v| v.position.y, f64::max, f64::NEG_INFINITY);
        assert!((right - size.width.as_mm()).abs() < 0.3, "{right}");
        assert!((bottom - size.height.as_mm()).abs() < 0.3, "{bottom}");
    }
}
