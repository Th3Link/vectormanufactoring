//! What the body of the Properties panel shows for the active tab
//! (`specs/0043-properties-tabs/` criteria 3, 10 and 23, `specs/0015-
//! document-size-and-rulers/` criterion 14a): the document's settings, the
//! Style area, or nothing.

use crate::panel_tabs::PanelTab;

/// The panel's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelContent {
    /// The Document section: the Document tab is active and the Pen has no
    /// unfinished path.
    Document,
    /// The Style area: the tool's scope holds objects to style.
    Style,
    /// Nothing: no heading, no text, no control (`specs/0017-style-panel-
    /// rework` criterion 1). The Document tab with an unfinished Pen path, so
    /// a resize would not move the path being drawn, or the Style tab with
    /// nothing in the tool's scope.
    Empty,
}

/// What the body shows for `active`. `style_has_objects` is whether the active
/// tool's style scope (`style_scope`) holds any object. The panel's width, the
/// canvas and the rulers never change with it.
///
/// Document shows the Document section unless the Pen has an unfinished path
/// (criterion 10). Style shows the Style area when the scope holds objects and
/// is empty otherwise, which only the Pen rule can cause (criterion 23).
#[must_use]
pub fn panel_body(
    active: PanelTab,
    pen_path_unfinished: bool,
    style_has_objects: bool,
) -> PanelContent {
    match active {
        PanelTab::Document if pen_path_unfinished => PanelContent::Empty,
        PanelTab::Document => PanelContent::Document,
        PanelTab::Style if style_has_objects => PanelContent::Style,
        PanelTab::Style => PanelContent::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PanelContent::{Document, Empty, Style};

    /// Criteria 3, 10 and 23 as a table: (active tab, Pen path unfinished,
    /// objects in scope) and the body.
    #[test]
    fn the_body_follows_the_active_tab_the_pen_and_the_scope() {
        let rows = [
            (PanelTab::Document, false, false, Document),
            (PanelTab::Document, false, true, Document),
            (PanelTab::Document, true, false, Empty),
            (PanelTab::Document, true, true, Empty),
            (PanelTab::Style, false, true, Style),
            (PanelTab::Style, true, true, Style),
            (PanelTab::Style, false, false, Empty),
            (PanelTab::Style, true, false, Empty),
        ];
        for (tab, pen, scope, want) in rows {
            assert_eq!(
                panel_body(tab, pen, scope),
                want,
                "{tab:?} pen {pen} scope {scope}"
            );
        }
    }
}
