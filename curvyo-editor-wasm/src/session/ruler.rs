//! `Session`'s read-only numbers and texts for the host's chrome in the display
//! unit: the ruler layout of the live view, the unit, and the status bar texts
//! (`specs/0015-document-size-and-rulers/` criteria 1 to 9, 12 and 21). The
//! frontend draws; every number and every label comes from here. PR 3 moves
//! the unit and the status texts to `session/document.rs` with the document
//! commands.

use curvyo_document_core::{DisplayUnit, Length};
use curvyo_ui_core::{RulerAxis, RulerLayout, format_cursor, format_size, ruler_layout};

use super::Session;

impl Session {
    /// The unit lengths are shown in.
    #[must_use]
    pub fn display_unit(&self) -> DisplayUnit {
        self.document.display_unit()
    }

    /// The tick layout of the ruler along `axis`, `length_px` long, for the
    /// live view, in the display unit. `digit_px` is the advance of one digit
    /// of the label font.
    #[must_use]
    pub fn ruler_layout(&self, axis: RulerAxis, length_px: f64, digit_px: f64) -> RulerLayout {
        ruler_layout(self.view(), axis, length_px, self.display_unit(), digit_px)
    }

    /// The status bar's cursor readout for a document point in millimetres,
    /// for example "x: 12.3  y: 45.6 mm" (criterion 21).
    #[must_use]
    pub fn cursor_text(&self, x_mm: f64, y_mm: f64) -> String {
        format_cursor(
            Length::from_mm(x_mm),
            Length::from_mm(y_mm),
            self.display_unit(),
        )
    }

    /// The status bar's size readout, for example "210.0 × 297.0 mm"
    /// (criteria 12 and 21).
    #[must_use]
    pub fn size_text(&self) -> String {
        format_size(self.document.size(), self.display_unit())
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{DocumentSize, Point};

    use super::*;

    /// Criterion 11a: after `show_default_view` the document corner is 72 px
    /// right of and below the canvas corner, and a click there is document
    /// (0, 0).
    #[test]
    fn the_default_view_puts_the_document_corner_72_px_in() {
        let mut session = Session::new(1);
        session.show_default_view();
        let corner = session.screen_to_document(72.0, 72.0);
        assert!(corner.x.abs() < 1e-9 && corner.y.abs() < 1e-9, "{corner:?}");
        let (x, y) = session.view().document_to_screen(Point::new(0.0, 0.0));
        assert!((x - 72.0).abs() < 1e-9 && (y - 72.0).abs() < 1e-9);
    }

    /// Criteria 3 and 9: the 0 tick is at the document corner, and a tick
    /// converts back to the document value it is labelled with.
    #[test]
    fn the_ruler_zero_is_at_the_document_corner_and_ticks_convert_back() {
        let mut session = Session::new(1);
        session.show_default_view();
        let layout = session.ruler_layout(RulerAxis::Horizontal, 800.0, 7.0);
        let origin = layout.origin_px.unwrap();
        assert!((origin - 72.0).abs() < 1e-9);
        // The tick labelled "100" (mm) at 100 % is where a click lands at
        // document x = 100 mm.
        let label = layout.labels.iter().find(|l| l.text == "100").unwrap();
        let document = session.screen_to_document(label.tick_px, 0.0);
        assert!((document.x - 100.0).abs() < 1e-9, "{document:?}");
    }

    /// Criteria 12 and 21: the status bar texts of a new project.
    #[test]
    fn the_status_texts_of_a_new_project_are_a4_in_mm() {
        let session = Session::new(1);
        assert_eq!(session.size_text(), "210.0 \u{d7} 297.0 mm");
        assert_eq!(session.cursor_text(12.34, 45.67), "x: 12.3  y: 45.7 mm");
        assert_eq!(session.document.size(), DocumentSize::default());
    }

    /// The unit follows the document: in inches the corner, the texts and the
    /// ruler step change, the document does not.
    #[test]
    fn the_display_unit_drives_the_ruler_and_the_texts() {
        let session = Session::new(1);
        assert!(session.document.set_display_unit(DisplayUnit::In));
        assert_eq!(session.display_unit(), DisplayUnit::In);
        assert_eq!(session.size_text(), "8.268 \u{d7} 11.693 in");
        let layout = session.ruler_layout(RulerAxis::Horizontal, 800.0, 7.0);
        assert!((layout.step() - 0.5).abs() < 1e-9, "{}", layout.step());
        assert_eq!(session.document.size(), DocumentSize::default());
    }
}
