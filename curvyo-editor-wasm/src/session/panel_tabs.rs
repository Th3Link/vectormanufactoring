//! `Session`'s glue for the Properties panel's tabs
//! (`specs/0043-properties-tabs/`, `adrs.md` decision 4): the active tab as
//! session view state, updated whenever the panel view is read, and the three
//! ways a tab is chosen (a press, Shift+Ctrl+F, Shift+Ctrl+D). The rules are
//! `curvyo_ui_core::panel_tabs`'s and `panel_content`'s.

use curvyo_ui_core::{
    PanelContent, PanelTab, PanelTabEntry, PanelTabs, StyleBlocker, panel_body, tab_entries_for,
};

use super::{Session, Tool};

/// What the Properties panel shows: the strip and the body of the active tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelView {
    /// What the body shows.
    pub content: PanelContent,
    /// The strip's tabs, in order.
    pub tabs: Vec<PanelTabEntry>,
}

impl Session {
    /// Whether the active tool's style scope holds objects: the "selection
    /// state" of the tab rules (`adrs.md` decision 2).
    fn panel_scope_non_empty(&self) -> bool {
        !self.style_scope().ids.is_empty()
    }

    /// Runs `change` on the tab state. The state is a `Cell` because a read of
    /// the panel is what notices a change of selection.
    fn update_panel_tabs<T>(&self, change: impl FnOnce(&mut PanelTabs) -> T) -> T {
        let mut tabs = self.panel_tabs.get();
        let result = change(&mut tabs);
        self.panel_tabs.set(tabs);
        result
    }

    /// Why the Style tab is dimmed, `None` while the scope holds objects: the
    /// tooltip names nothing selected, or the tool that cannot style the
    /// selection.
    fn style_blocker(&self, non_empty: bool) -> Option<StyleBlocker> {
        if non_empty {
            return None;
        }
        Some(match self.tool() {
            Tool::Pen if !self.selection.is_empty() => StyleBlocker::PenTool,
            Tool::Node if !self.selection.is_empty() => StyleBlocker::NodeToolWithoutPath,
            _ => StyleBlocker::NothingSelected,
        })
    }

    /// Reads the panel: lets the tab rule see the scope's current state (so an
    /// edge since the last read switches the tab, in the same frame as the
    /// selection change), then returns the strip and the body.
    pub fn panel_view(&self) -> PanelView {
        let non_empty = self.panel_scope_non_empty();
        let active = self.update_panel_tabs(|tabs| {
            tabs.observe(non_empty);
            tabs.active()
        });
        PanelView {
            content: panel_body(active, self.pen_path_unfinished(), non_empty),
            tabs: tab_entries_for(active, self.style_blocker(non_empty)),
        }
    }

    /// What the panel's body shows ([`Session::panel_view`]'s `content`).
    pub fn panel_content(&self) -> PanelContent {
        self.panel_view().content
    }

    /// A press on the tab `name`: `false` for an unknown name and for the
    /// dimmed Style tab, which does nothing (criterion 4).
    pub fn press_panel_tab(&mut self, name: &str) -> bool {
        let Some(tab) = PanelTab::from_name(name) else {
            return false;
        };
        let non_empty = self.panel_scope_non_empty();
        self.update_panel_tabs(|tabs| {
            tabs.observe(non_empty);
            tabs.press(tab, non_empty)
        })
    }

    /// Shift+Ctrl+F (criterion 14): Style with objects in scope, else Document.
    pub fn panel_shortcut_style(&mut self) {
        let non_empty = self.panel_scope_non_empty();
        self.update_panel_tabs(|tabs| {
            tabs.observe(non_empty);
            tabs.shortcut_style(non_empty);
        });
    }

    /// Shift+Ctrl+D (criterion 14): the Document tab.
    pub fn panel_shortcut_document(&mut self) {
        let non_empty = self.panel_scope_non_empty();
        self.update_panel_tabs(|tabs| {
            tabs.observe(non_empty);
            tabs.shortcut_document();
        });
    }
}

/// [`PanelView`] as the flat record the host reads (ADR 0001 §5): the body's
/// name, the active tab and one entry per tab in order.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelTabsRecord {
    /// `"document"`, `"style"` or `"empty"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub content: String,
    /// The active tab's name.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub active: String,
    /// The name of each tab.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub tab_names: Vec<String>,
    /// The accessible name of each tab.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub tab_labels: Vec<String>,
    /// The tooltip of each tab.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub tab_tooltips: Vec<String>,
    /// `1` for a tab that can be pressed, `0` for the dimmed one.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub tab_enabled: Vec<u8>,
}

impl PanelTabsRecord {
    /// The record of `view`.
    #[must_use]
    pub fn new(view: &PanelView) -> Self {
        let content = match view.content {
            PanelContent::Document => "document",
            PanelContent::Style => "style",
            PanelContent::Empty => "empty",
        };
        Self {
            content: content.to_string(),
            active: view
                .tabs
                .iter()
                .find(|tab| tab.selected)
                .map_or_else(String::new, |tab| tab.name.to_string()),
            tab_names: view.tabs.iter().map(|t| t.name.to_string()).collect(),
            tab_labels: view.tabs.iter().map(|t| t.label.to_string()).collect(),
            tab_tooltips: view.tabs.iter().map(|t| t.tooltip.to_string()).collect(),
            tab_enabled: view.tabs.iter().map(|t| u8::from(t.enabled)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{Length, NodeId, Point, RectBounds};

    use super::super::document::{DocumentSide, SizeOutcome};
    use super::*;

    fn rect(session: &Session, x: f64) -> NodeId {
        session.document.create_rect(RectBounds {
            origin: Point::new(x, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        })
    }

    fn active(session: &mut Session) -> String {
        PanelTabsRecord::new(&session.panel_view()).active
    }

    /// Criteria 5 and 6: a new session shows Document; a selection brings
    /// Style, clearing it brings Document back.
    #[test]
    fn the_tab_follows_the_selection_edges() {
        let mut session = Session::new(1);
        assert_eq!(active(&mut session), "document");
        let id = rect(&session, 0.0);
        session.selection.select_single(id);
        assert_eq!(active(&mut session), "style");
        assert_eq!(session.panel_content(), PanelContent::Style);
        session.selection.clear();
        assert_eq!(active(&mut session), "document");
    }

    /// Criterion 8: a pressed Document stays through a change of selection and
    /// gives way to Style at the next empty to non-empty edge.
    #[test]
    fn a_pressed_document_tab_is_sticky_until_the_selection_empties() {
        let mut session = Session::new(1);
        let (a, b) = (rect(&session, 0.0), rect(&session, 30.0));
        session.selection.select_single(a);
        assert_eq!(active(&mut session), "style");
        assert!(session.press_panel_tab("document"));
        assert_eq!(session.panel_content(), PanelContent::Document);
        session.selection.select_single(b);
        assert_eq!(active(&mut session), "document");
        session.selection.clear();
        assert_eq!(active(&mut session), "document");
        session.selection.select_single(a);
        assert_eq!(active(&mut session), "style");
    }

    /// Criterion 4: Style cannot be pressed with nothing selected.
    #[test]
    fn the_dimmed_style_tab_does_nothing() {
        let mut session = Session::new(1);
        assert!(!session.press_panel_tab("style"));
        assert!(!session.press_panel_tab("history"));
        let record = PanelTabsRecord::new(&session.panel_view());
        assert_eq!(record.tab_names, ["document", "style"]);
        assert_eq!(record.tab_enabled, [1, 0]);
        assert_eq!(record.tab_tooltips[1], "Style: select an object first");
        assert_eq!(record.active, "document");
    }

    /// Criterion 4: the dimmed Style tab says why, also when something is
    /// selected but the tool cannot style it.
    #[test]
    fn the_dimmed_style_tooltip_names_the_reason() {
        let mut session = Session::new(1);
        let id = rect(&session, 0.0);
        session.selection.select_single(id);
        session.set_tool(Tool::Node);
        let tip = |s: &Session| s.panel_view().tabs[1].tooltip;
        assert_eq!(tip(&session), "Style: the Node tool styles paths only");
        session.set_tool(Tool::Pen);
        assert_eq!(tip(&session), "Style: the Pen tool has nothing to style");
        session.set_tool(Tool::Select);
        assert_eq!(tip(&session), "Style (Shift+Ctrl+F)");
        session.selection.clear();
        assert_eq!(tip(&session), "Style: select an object first");
    }

    /// Criterion 14: the two shortcuts.
    #[test]
    fn the_shortcuts_pick_style_or_document() {
        let mut session = Session::new(1);
        session.panel_shortcut_style();
        assert_eq!(active(&mut session), "document");
        let id = rect(&session, 0.0);
        session.selection.select_single(id);
        assert!(session.press_panel_tab("document"));
        session.panel_shortcut_style();
        assert_eq!(active(&mut session), "style");
        session.panel_shortcut_document();
        assert_eq!(active(&mut session), "document");
    }

    /// Criterion 10: the strip stays during an unfinished Pen path, both
    /// bodies are empty, and a press still sets the tab.
    #[test]
    fn an_unfinished_pen_path_empties_both_bodies_but_not_the_strip() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        for point in [Point::new(10.0, 10.0), Point::new(50.0, 40.0)] {
            session.pointer_down(point, false);
            session.pointer_up(point, false, false);
        }
        let view = session.panel_view();
        assert_eq!(view.content, PanelContent::Empty);
        assert_eq!(view.tabs.len(), 2);
        assert!(session.press_panel_tab("document"));
        assert_eq!(session.panel_content(), PanelContent::Empty);
        session.finish_pen();
        assert_eq!(session.panel_content(), PanelContent::Document);
    }

    /// Criterion 6 with the Pen (`adrs.md` decision 2): switching to the Pen
    /// with a selection is an edge to empty, so Style hands over to Document.
    #[test]
    fn switching_to_the_pen_with_a_selection_shows_the_document() {
        let mut session = Session::new(1);
        let id = rect(&session, 0.0);
        session.selection.select_single(id);
        assert_eq!(active(&mut session), "style");
        session.set_tool(Tool::Pen);
        assert_eq!(session.panel_content(), PanelContent::Document);
    }

    /// Criterion 24: the document can be resized with an object selected, and
    /// the selection stays.
    #[test]
    fn the_size_is_editable_while_an_object_stays_selected() {
        let mut session = Session::new(1);
        let id = rect(&session, 10.0);
        session.selection.select_single(id);
        assert!(session.press_panel_tab("document"));
        assert_eq!(session.panel_content(), PanelContent::Document);
        assert_eq!(
            session.set_document_side(DocumentSide::Width, "300"),
            SizeOutcome::Committed
        );
        assert_eq!(session.selection.ids(), [id]);
        assert_eq!(session.panel_content(), PanelContent::Document);
    }

    /// Criterion 11: a new session starts over.
    #[test]
    fn a_new_session_resets_the_choice() {
        let mut session = Session::new(1);
        let id = rect(&session, 0.0);
        session.selection.select_single(id);
        assert!(session.press_panel_tab("document"));
        let mut fresh = Session::new(2);
        assert_eq!(active(&mut fresh), "document");
    }
}
