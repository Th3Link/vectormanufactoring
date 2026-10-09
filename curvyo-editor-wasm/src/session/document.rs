//! The document's own settings (size and display unit) as the Properties panel
//! and the status bar show and change them (`specs/0015-document-size-and-
//! rulers/` criteria 12, 14 to 27a, 33 to 36).
//!
//! A resize or a fit moves every object; the view moves by the same shift, so
//! nothing moves on screen and the document's edges move instead
//! (criterion 20).

use curvyo_document_core::{DisplayUnit, Document, DocumentSize, DocumentSizeError, Length};
use curvyo_ui_core::{
    PanelContent, content_bounds, content_too_large_message, document_side_message, format_cursor,
    format_field_length, format_size, panel_content, parse_document_side,
};

use super::Session;

/// A side of the document: the Width and Height fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSide {
    /// The document's width.
    Width,
    /// The document's height.
    Height,
}

/// What a typed size did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeOutcome {
    /// The size was applied as one commit.
    Committed,
    /// The text names the size the document already has: nothing was written.
    Unchanged,
    /// Not a number, or outside 1 mm to 100 000 mm: nothing was written.
    Invalid,
}

/// What Fit to content did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FitOutcome {
    /// The size and positions changed, as one commit.
    Fitted,
    /// The document already fits: nothing was written.
    AlreadyFits,
    /// The document has no objects, or none with usable geometry: nothing was
    /// written.
    Empty,
    /// The content is larger than the largest document: nothing was written.
    /// Carries the message, with the limit in the display unit.
    TooLarge(String),
    /// The Pen has an unfinished path, which a fit would not move: nothing was
    /// written (criterion 14a).
    Blocked,
}

impl Session {
    /// What the Properties panel shows (criterion 14a).
    #[must_use]
    pub fn panel_content(&self) -> PanelContent {
        panel_content(
            &self.selection,
            self.pen_path_unfinished(),
            !self.style_scope().ids.is_empty(),
        )
    }

    /// Whether the Pen holds an unfinished path, whichever tool is active. A
    /// resize or a fit moves the committed objects and would leave that path
    /// behind, so neither runs while it exists (criterion 14a).
    fn pen_path_unfinished(&self) -> bool {
        self.pen.in_progress_nodes().is_some()
    }

    /// The unit lengths are shown in.
    #[must_use]
    pub fn display_unit(&self) -> DisplayUnit {
        self.document.display_unit()
    }

    /// Sets the display unit, one commit that moves nothing (criterion 36).
    /// Returns whether it changed.
    pub fn set_display_unit(&mut self, unit: DisplayUnit) -> bool {
        self.document.set_display_unit(unit)
    }

    /// Whether the document has any object: Fit to content is offered only
    /// then (criterion 22).
    #[must_use]
    pub fn has_objects(&self) -> bool {
        !self.document.object_ids().is_empty()
    }

    /// The text of a size field: the side in the display unit, 3 decimals in
    /// mm and 4 in cm and in, no trailing zeros (criterion 35). Display only.
    #[must_use]
    pub fn side_text(&self, side: DocumentSide) -> String {
        let current = self.document.size();
        let length = match side {
            DocumentSide::Width => current.width,
            DocumentSide::Height => current.height,
        };
        format_field_length(length, self.display_unit())
    }

    /// The message a refused size shows, the limits in the display unit.
    #[must_use]
    pub fn side_message(&self) -> String {
        document_side_message(self.display_unit())
    }

    /// Applies the text typed in a size field (criteria 15 to 19): parsed in
    /// the display unit, the other side kept, one commit that moves every
    /// object by half the change. The view follows by the same shift.
    pub fn set_document_side(&mut self, side: DocumentSide, text: &str) -> SizeOutcome {
        if self.pen_path_unfinished() {
            return SizeOutcome::Unchanged;
        }
        let Some(typed) = parse_document_side(text, self.display_unit()) else {
            return SizeOutcome::Invalid;
        };
        let before = self.document.size();
        let wanted = match side {
            DocumentSide::Width => DocumentSize::new(typed, before.height),
            DocumentSide::Height => DocumentSize::new(before.width, typed),
        };
        match self.document.resize(wanted) {
            Ok(true) => {
                self.follow_document_shift(Document::resize_shift(before, self.document.size()));
                SizeOutcome::Committed
            }
            Ok(false) => SizeOutcome::Unchanged,
            Err(_) => SizeOutcome::Invalid,
        }
    }

    /// Fits the document to the extent of all objects (criteria 22 to 27a).
    pub fn fit_document(&mut self) -> FitOutcome {
        if self.pen_path_unfinished() {
            return FitOutcome::Blocked;
        }
        let Some(bounds) = content_bounds(&self.document) else {
            return FitOutcome::Empty;
        };
        let shift = Document::fit_shift(bounds);
        match self.document.fit_to_content(bounds) {
            Ok(true) => {
                self.follow_document_shift(shift);
                FitOutcome::Fitted
            }
            Ok(false) => FitOutcome::AlreadyFits,
            Err(DocumentSizeError::OutOfRange) => {
                FitOutcome::TooLarge(content_too_large_message(self.display_unit()))
            }
            // A box that is not finite (a damaged file) is not "too large".
            Err(_) => FitOutcome::Empty,
        }
    }

    /// Moves the view by the shift every object just moved by, so the objects
    /// stay where they were on screen (criterion 20).
    fn follow_document_shift(&mut self, shift: curvyo_document_core::Vec2) {
        self.viewport.pan_by_document_offset(shift);
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
    use curvyo_document_core::{Length, Point, RectBounds};

    use super::*;

    fn with_rect(
        session: &Session,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> curvyo_document_core::NodeId {
        session.document.create_rect(RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        })
    }

    /// Criteria 12 and 21: the status bar texts of a new project.
    #[test]
    fn the_status_texts_of_a_new_project_are_a4_in_mm() {
        let session = Session::new(1);
        assert_eq!(session.size_text(), "210.0 \u{d7} 297.0 mm");
        assert_eq!(session.cursor_text(12.34, 45.67), "x: 12.3  y: 45.7 mm");
        assert_eq!(session.side_text(DocumentSide::Width), "210");
        assert_eq!(session.side_text(DocumentSide::Height), "297");
    }

    /// Criteria 14a, 22: the panel's content and the Fit button's presence.
    #[test]
    fn the_panel_shows_the_document_with_nothing_selected() {
        let mut session = Session::new(1);
        assert_eq!(session.panel_content(), PanelContent::Document);
        assert!(!session.has_objects());
        with_rect(&session, 0.0, 0.0, 10.0, 10.0);
        assert!(session.has_objects());
        session
            .selection
            .select_single(session.document.object_ids()[0]);
        assert_eq!(session.panel_content(), PanelContent::Style);
        session.selection.clear();
        assert_eq!(session.panel_content(), PanelContent::Document);
    }

    /// Criterion 14a: an unfinished Pen path empties the panel.
    #[test]
    fn an_unfinished_pen_path_empties_the_panel() {
        let mut session = Session::new(1);
        session.set_tool(super::super::Tool::Pen);
        for point in [Point::new(10.0, 10.0), Point::new(50.0, 40.0)] {
            session.pointer_down(point, false);
            session.pointer_up(point, false, false);
        }
        assert_eq!(session.panel_content(), PanelContent::Empty);
        session.finish_pen();
        assert_eq!(session.panel_content(), PanelContent::Document);
    }

    /// Criteria 15 to 20 through the session: the spec's example, the view
    /// follows, the object stays where it was on screen.
    #[test]
    fn a_typed_width_resizes_with_the_centre_fixed_and_the_view_follows() {
        let mut session = Session::new(1);
        session.show_default_view();
        let id = with_rect(&session, 10.0, 10.0, 50.0, 50.0);
        let screen_before = session.view().document_to_screen(Point::new(10.0, 10.0));

        assert_eq!(
            session.set_document_side(DocumentSide::Width, "300"),
            SizeOutcome::Committed
        );
        assert_eq!(
            session.set_document_side(DocumentSide::Height, "400"),
            SizeOutcome::Committed
        );

        assert_eq!(session.size_text(), "300.0 \u{d7} 400.0 mm");
        let Some(curvyo_document_core::ObjectSnapshot::Primitive(p)) = session.document.object(id)
        else {
            panic!("a primitive");
        };
        let curvyo_document_core::Shape::Rect { bounds, .. } = p.shape else {
            panic!("a rect");
        };
        assert!((bounds.origin.x - 55.0).abs() < 1e-9 && (bounds.origin.y - 61.5).abs() < 1e-9);
        let screen_after = session.view().document_to_screen(bounds.origin);
        assert!(
            (screen_after.0 - screen_before.0).abs() < 0.5
                && (screen_after.1 - screen_before.1).abs() < 0.5,
            "{screen_before:?} {screen_after:?}"
        );
    }

    /// Criterion 16: bad text and a same-size value write nothing.
    #[test]
    fn bad_and_unchanged_sizes_write_nothing() {
        let mut session = Session::new(1);
        let commits = session.document.export_json().unwrap();
        for text in ["", "abc", "0", "100001", "21cm", "-5"] {
            assert_eq!(
                session.set_document_side(DocumentSide::Width, text),
                SizeOutcome::Invalid,
                "{text:?}"
            );
        }
        assert_eq!(
            session.set_document_side(DocumentSide::Width, "210"),
            SizeOutcome::Unchanged
        );
        assert_eq!(session.document.export_json().unwrap(), commits);
        assert_eq!(session.side_message(), "Enter a number from 1 to 100000");
    }

    /// Criteria 35, 36: inches, exact; a unit change moves nothing and the
    /// fields follow.
    #[test]
    fn inches_set_the_exact_millimetres_and_the_unit_is_a_display_change() {
        let mut session = Session::new(1);
        assert!(session.set_display_unit(DisplayUnit::In));
        assert_eq!(
            session.set_document_side(DocumentSide::Width, "8.5"),
            SizeOutcome::Committed
        );
        assert_eq!(
            session.set_document_side(DocumentSide::Height, "11"),
            SizeOutcome::Committed
        );
        assert_eq!(session.document.size(), DocumentSize::from_mm(215.9, 279.4));
        assert_eq!(session.side_text(DocumentSide::Width), "8.5");
        assert_eq!(session.side_message(), "Enter a number from 0.04 to 3937");
        assert!(session.set_display_unit(DisplayUnit::Mm));
        assert_eq!(session.side_text(DocumentSide::Width), "215.9");
        assert!(!session.set_display_unit(DisplayUnit::Mm));
    }

    /// Criteria 22 to 27a: Fit, with the view following, then "already fits",
    /// the empty document, and content that is too large.
    #[test]
    fn fit_follows_with_the_view_and_reports_what_it_did() {
        let mut session = Session::new(1);
        session.show_default_view();
        assert_eq!(session.fit_document(), FitOutcome::Empty);
        let id = with_rect(&session, 30.0, 40.0, 20.0, 10.0);
        let before = session.view().document_to_screen(Point::new(30.0, 40.0));

        assert_eq!(session.fit_document(), FitOutcome::Fitted);
        assert_eq!(session.document.size(), DocumentSize::from_mm(20.0, 10.0));
        let after = session.view().document_to_screen(Point::new(0.0, 0.0));
        assert!((after.0 - before.0).abs() < 0.5 && (after.1 - before.1).abs() < 0.5);
        assert_eq!(session.fit_document(), FitOutcome::AlreadyFits);

        with_rect(&session, 0.0, 0.0, 150_000.0, 10.0);
        let outcome = session.fit_document();
        assert!(
            matches!(&outcome, FitOutcome::TooLarge(m) if m.contains("100000 mm") && m.ends_with("Nothing was changed.")),
            "{outcome:?}"
        );
        let _ = id;
    }

    /// Criteria 36 and 39: size, unit and positions survive Save, Close and
    /// Open, and an opened file starts with the document 72 px in.
    #[test]
    fn size_unit_and_positions_survive_save_and_open() {
        let mut session = Session::new(1);
        let id = with_rect(&session, 10.0, 10.0, 50.0, 50.0);
        assert!(session.set_display_unit(DisplayUnit::In));
        assert_eq!(
            session.set_document_side(DocumentSide::Width, "8.5"),
            SizeOutcome::Committed
        );
        assert_eq!(session.fit_document(), FitOutcome::Fitted);
        assert_eq!(
            session.set_document_side(DocumentSide::Height, "4"),
            SizeOutcome::Committed
        );

        let bytes = session.pack("0.1.0").unwrap();
        let mut reopened = Session::open(2, &bytes).unwrap();
        reopened.show_default_view();

        assert_eq!(reopened.display_unit(), DisplayUnit::In);
        assert_eq!(reopened.document.size(), session.document.size());
        assert_eq!(reopened.document.object(id), session.document.object(id));
        assert_eq!(reopened.size_text(), session.size_text());
    }

    /// Criterion 14a: no resize and no fit while the Pen holds a path, and
    /// leaving the Pen ends the path as drawn, so none stays behind.
    #[test]
    fn nothing_resizes_while_the_pen_holds_a_path_and_leaving_the_pen_ends_it() {
        let mut session = Session::new(1);
        let rect_id = with_rect(&session, 10.0, 10.0, 50.0, 50.0);
        let before = session.document.object(rect_id);
        session.set_tool(super::super::Tool::Pen);
        for point in [Point::new(100.0, 100.0), Point::new(150.0, 120.0)] {
            session.pointer_down(point, false);
            session.pointer_up(point, false, false);
        }
        assert_eq!(
            session.set_document_side(DocumentSide::Width, "300"),
            SizeOutcome::Unchanged
        );
        assert_eq!(session.fit_document(), FitOutcome::Blocked);
        assert_eq!(session.document.size(), DocumentSize::default());
        assert_eq!(session.document.object(rect_id), before);

        // Switching away ends the path as drawn: one more object, no hidden
        // path, and the panel offers the Document section.
        session.set_tool(super::super::Tool::Select);
        assert_eq!(session.document.object_ids().len(), 2);
        assert_eq!(session.panel_content(), PanelContent::Document);
        session.set_tool(super::super::Tool::Pen);
        assert_eq!(session.panel_content(), PanelContent::Document);
        assert_eq!(
            session.set_document_side(DocumentSide::Width, "300"),
            SizeOutcome::Committed
        );

        // A lone node is dropped, not committed.
        let mut lone = Session::new(2);
        lone.set_tool(super::super::Tool::Pen);
        lone.pointer_down(Point::new(5.0, 5.0), false);
        lone.pointer_up(Point::new(5.0, 5.0), false, false);
        lone.set_tool(super::super::Tool::Select);
        assert_eq!(lone.document.object_ids().len(), 0);
        assert_eq!(lone.panel_content(), PanelContent::Document);
    }
}
