//! What the Properties panel shows (`specs/0015-document-size-and-rulers/`
//! criterion 14a): the document's settings, the Style area, or nothing.

use crate::object_selection::ObjectSelection;

/// The panel's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelContent {
    /// The Document section: nothing is selected and the Pen has no
    /// unfinished path.
    Document,
    /// The Style area: the tool's scope holds objects to style.
    Style,
    /// Nothing: no heading, no text, no control (`specs/0017-style-panel-
    /// rework` criterion 1). The Pen is active, or the Node tool has no path
    /// to style, or the Pen has an unfinished path and nothing is selected, so
    /// a resize would move the committed objects but not the path being drawn.
    Empty,
}

/// What the panel shows. `style_has_objects` is whether the active tool's
/// style scope (`style_scope`) holds any object. The panel's width, the canvas
/// and the rulers never change with it.
///
/// With objects in scope the Style area shows. Otherwise, with nothing
/// selected and no unfinished Pen path, the Document section shows; in every
/// other case the body is empty (a selection the tool cannot style, the Pen
/// tool, a Node tool without a path).
#[must_use]
pub fn panel_content(
    selection: &ObjectSelection,
    pen_path_unfinished: bool,
    style_has_objects: bool,
) -> PanelContent {
    if style_has_objects {
        PanelContent::Style
    } else if selection.is_empty() && !pen_path_unfinished {
        PanelContent::Document
    } else {
        PanelContent::Empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_in_scope_show_style_whatever_the_selection_or_pen_say() {
        use curvyo_document_core::{Document, Length, Point, RectBounds};

        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(5.0),
            height: Length::from_mm(5.0),
        });
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        assert_eq!(panel_content(&selection, false, true), PanelContent::Style);
        assert_eq!(panel_content(&selection, true, true), PanelContent::Style);
        // The Node tool with a selected node but no object selection.
        assert_eq!(
            panel_content(&ObjectSelection::new(), false, true),
            PanelContent::Style
        );
    }

    #[test]
    fn a_selection_the_tool_cannot_style_leaves_the_body_empty() {
        use curvyo_document_core::{Document, Length, Point, RectBounds};

        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(5.0),
            height: Length::from_mm(5.0),
        });
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        assert_eq!(panel_content(&selection, false, false), PanelContent::Empty);
    }

    #[test]
    fn nothing_selected_shows_the_document_unless_the_pen_has_an_open_path() {
        let nothing = ObjectSelection::new();
        assert_eq!(
            panel_content(&nothing, false, false),
            PanelContent::Document
        );
        assert_eq!(panel_content(&nothing, true, false), PanelContent::Empty);
    }
}
