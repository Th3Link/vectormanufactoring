//! `Session`'s ruler layout for the live view and the display unit
//! (`specs/0015-document-size-and-rulers/` criteria 1 to 9). The frontend
//! draws; every number and every label comes from here.

use curvyo_ui_core::{RulerAxis, RulerLayout, ruler_layout};

use super::Session;

impl Session {
    /// The tick layout of the ruler along `axis`, `length_px` long, for the
    /// live view, in the display unit. `digit_px` and `minus_px` are the
    /// widest digit's and the minus sign's advance in the label font.
    #[must_use]
    pub fn ruler_layout(
        &self,
        axis: RulerAxis,
        length_px: f64,
        digit_px: f64,
        minus_px: f64,
    ) -> RulerLayout {
        ruler_layout(
            self.view(),
            axis,
            length_px,
            self.display_unit(),
            digit_px,
            minus_px,
        )
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{DisplayUnit, DocumentSize, Point};

    use super::*;

    /// Criterion 11a: after `show_default_view` the document corner is 128 px
    /// right of and below the canvas corner, and a click there is document
    /// (0, 0).
    #[test]
    fn the_default_view_puts_the_document_corner_128_px_in() {
        let mut session = Session::new(1);
        session.show_default_view();
        let corner = session.screen_to_document(128.0, 128.0);
        assert!(corner.x.abs() < 1e-9 && corner.y.abs() < 1e-9, "{corner:?}");
        let (x, y) = session.view().document_to_screen(Point::new(0.0, 0.0));
        assert!((x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9);
    }

    /// Criteria 3 and 9: the 0 tick is at the document corner, and a tick
    /// converts back to the document value it is labelled with.
    #[test]
    fn the_ruler_zero_is_at_the_document_corner_and_ticks_convert_back() {
        let mut session = Session::new(1);
        session.show_default_view();
        let layout = session.ruler_layout(RulerAxis::Horizontal, 800.0, 7.0, 7.0);
        let origin = layout.origin_px.unwrap();
        assert!((origin - 128.0).abs() < 1e-9);
        // The tick labelled "100" (mm) at 100 % is where a click lands at
        // document x = 100 mm.
        let label = layout.labels.iter().find(|l| l.text == "100").unwrap();
        let document = session.screen_to_document(label.tick_px, 0.0);
        assert!((document.x - 100.0).abs() < 1e-9, "{document:?}");
    }

    /// The unit follows the document: in inches the corner, the texts and the
    /// ruler step change, the document does not.
    #[test]
    fn the_display_unit_drives_the_ruler_and_the_texts() {
        let session = Session::new(1);
        assert!(session.document.set_display_unit(DisplayUnit::In));
        assert_eq!(session.display_unit(), DisplayUnit::In);
        assert_eq!(session.size_text(), "8.268 \u{d7} 11.693 in");
        let layout = session.ruler_layout(RulerAxis::Horizontal, 800.0, 7.0, 7.0);
        assert!((layout.step() - 0.5).abs() < 1e-9, "{}", layout.step());
        assert_eq!(session.document.size(), DocumentSize::default());
    }
}
