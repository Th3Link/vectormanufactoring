//! What the Properties panel shows (`specs/0015-document-size-and-rulers/`
//! criterion 14a): the document's settings, the Style area, or nothing.

use crate::object_selection::ObjectSelection;

/// The panel's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelContent {
    /// The Document section: nothing is selected and the Pen has no
    /// unfinished path.
    Document,
    /// The Style area: one or more objects are selected.
    Style,
    /// Nothing: the Pen has an unfinished path and nothing is selected, so a
    /// resize would move the committed objects but not the path being drawn.
    Empty,
}

/// What the panel shows for `selection` and whether the Pen has an unfinished
/// path. The panel's width, the canvas and the rulers never change with it.
#[must_use]
pub fn panel_content(selection: &ObjectSelection, pen_path_unfinished: bool) -> PanelContent {
    if !selection.is_empty() {
        PanelContent::Style
    } else if pen_path_unfinished {
        PanelContent::Empty
    } else {
        PanelContent::Document
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A selection shows Style, also while the Pen has an unfinished path
    /// (the Style area says "finish the path to style it" as before).
    #[test]
    fn a_selection_shows_style_whatever_the_pen_does() {
        use curvyo_document_core::{Document, Length, Point, RectBounds};

        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(5.0),
            height: Length::from_mm(5.0),
        });
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        assert_eq!(panel_content(&selection, false), PanelContent::Style);
        assert_eq!(panel_content(&selection, true), PanelContent::Style);
        selection.clear();
        assert_eq!(panel_content(&selection, false), PanelContent::Document);
    }

    #[test]
    fn nothing_selected_shows_the_document_unless_the_pen_has_an_open_path() {
        let nothing = ObjectSelection::new();
        assert_eq!(panel_content(&nothing, false), PanelContent::Document);
        assert_eq!(panel_content(&nothing, true), PanelContent::Empty);
    }
}
