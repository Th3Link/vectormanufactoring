//! Which tab of the Properties panel is active, and when it changes by itself
//! (`specs/0043-properties-tabs/` criteria 4 to 8 and 14), plus the strip's
//! entries: names, tooltips and which tab is dimmed.

/// A tab of the Properties panel. The History tab arrives with `0020`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTab {
    /// The document's own settings.
    Document,
    /// The style of the objects in the active tool's scope.
    Style,
}

impl PanelTab {
    /// The tab's name, which the host sends back on a press.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Style => "style",
        }
    }

    /// The tab called `name`; `None` for an unknown name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        TABS.iter().copied().find(|tab| tab.name() == name)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Document => "Document",
            Self::Style => "Style",
        }
    }

    fn tooltip(self, blocker: Option<StyleBlocker>) -> &'static str {
        match (self, blocker) {
            (Self::Document, _) => "Document (Shift+Ctrl+D)",
            (Self::Style, None) => "Style (Shift+Ctrl+F)",
            (Self::Style, Some(blocker)) => blocker.tooltip(),
        }
    }
}

/// Why the Style tab is dimmed: the tool's style scope holds no object. The
/// tooltip names the right reason (criterion 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleBlocker {
    /// Nothing is selected.
    NothingSelected,
    /// Something is selected, but the Pen tool styles nothing.
    PenTool,
    /// Something is selected, but the Node tool styles paths only and the
    /// selection holds none.
    NodeToolWithoutPath,
}

impl StyleBlocker {
    const fn tooltip(self) -> &'static str {
        match self {
            Self::NothingSelected => "Style: select an object first",
            Self::PenTool => "Style: the Pen tool has nothing to style",
            Self::NodeToolWithoutPath => "Style: the Node tool styles paths only",
        }
    }
}

/// The strip's tabs, in order (criterion 1).
const TABS: [PanelTab; 2] = [PanelTab::Document, PanelTab::Style];

/// The active tab and the one fact the automatic switches need: whether the
/// style scope held objects when the panel was last read. Switches act on the
/// edges of that fact only, so a tab the maker pressed survives every change
/// that is not an edge (criterion 8) and a tab that is never switched
/// automatically (History, later) needs no rule of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelTabs {
    active: PanelTab,
    scope_was_non_empty: bool,
}

impl PanelTabs {
    /// A new session: Document with an empty scope, Style with objects in it
    /// (criterion 5).
    #[must_use]
    pub fn new(non_empty: bool) -> Self {
        Self {
            active: if non_empty {
                PanelTab::Style
            } else {
                PanelTab::Document
            },
            scope_was_non_empty: non_empty,
        }
    }

    /// The active tab.
    #[must_use]
    pub const fn active(&self) -> PanelTab {
        self.active
    }

    /// The scope is now empty or not (criteria 6 to 8): from empty to not
    /// empty Document hands over to Style, from not empty to empty Style hands
    /// over to Document, anything else keeps the tab.
    pub fn observe(&mut self, non_empty: bool) {
        match (self.scope_was_non_empty, non_empty, self.active) {
            (false, true, PanelTab::Document) => self.active = PanelTab::Style,
            (true, false, PanelTab::Style) => self.active = PanelTab::Document,
            _ => {}
        }
        self.scope_was_non_empty = non_empty;
    }

    /// A press on `tab`. `false` and no change for Style with an empty scope
    /// (criterion 4); the caller observes the scope first.
    pub fn press(&mut self, tab: PanelTab, non_empty: bool) -> bool {
        if tab == PanelTab::Style && !non_empty {
            return false;
        }
        self.active = tab;
        true
    }

    /// Shift+Ctrl+F (criterion 14): Style with objects in scope, Document
    /// without.
    pub fn shortcut_style(&mut self, non_empty: bool) {
        self.active = if non_empty {
            PanelTab::Style
        } else {
            PanelTab::Document
        };
    }

    /// Shift+Ctrl+D (criterion 14): Document, whatever the scope.
    pub fn shortcut_document(&mut self) {
        self.active = PanelTab::Document;
    }
}

/// One tab of the strip as the host draws it (criteria 1 to 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelTabEntry {
    /// The name the host sends back on a press.
    pub name: &'static str,
    /// The accessible name.
    pub label: &'static str,
    /// The text-only tooltip.
    pub tooltip: &'static str,
    /// `false` for the dimmed Style tab with nothing in scope.
    pub enabled: bool,
    /// Whether this is the active tab.
    pub selected: bool,
}

/// The strip for the current state, in order. With `non_empty` false the
/// reason is that nothing is selected.
#[must_use]
pub fn tab_entries(active: PanelTab, non_empty: bool) -> Vec<PanelTabEntry> {
    tab_entries_for(
        active,
        (!non_empty).then_some(StyleBlocker::NothingSelected),
    )
}

/// The strip when the Style tab is dimmed for `blocker` (`None`: it is not).
#[must_use]
pub fn tab_entries_for(active: PanelTab, blocker: Option<StyleBlocker>) -> Vec<PanelTabEntry> {
    TABS.iter()
        .map(|&tab| PanelTabEntry {
            name: tab.name(),
            label: tab.label(),
            tooltip: tab.tooltip(blocker),
            enabled: tab != PanelTab::Style || blocker.is_none(),
            selected: tab == active,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use PanelTab::{Document, Style};

    /// One step of the table: what happens to the scope or what the maker does.
    #[derive(Clone, Copy)]
    enum Step {
        /// The scope becomes empty (`false`) or not empty (`true`).
        Scope(bool),
        Press(PanelTab),
        ShortcutStyle,
        ShortcutDocument,
    }
    use Step::{Press, Scope, ShortcutDocument, ShortcutStyle};

    /// Criteria 5 to 8 and 14, row by row: the start state, the steps, the
    /// tab after each step.
    #[test]
    fn the_active_tab_follows_the_rules_of_criteria_5_to_8_and_14() {
        let rows: &[(bool, &[(Step, PanelTab)])] = &[
            // 5: a new session.
            (false, &[]),
            (true, &[]),
            // 6: the edges while the tab follows the scope.
            (
                false,
                &[
                    (Scope(true), Style),
                    (Scope(false), Document),
                    (Scope(true), Style),
                ],
            ),
            // 7: a change that stays not empty keeps the tab.
            (
                true,
                &[
                    (Scope(true), Style),
                    (Press(Document), Document),
                    (Scope(true), Document),
                ],
            ),
            // 8: a pressed Document survives non-empty to non-empty and falls
            // back to Style at the next empty to non-empty edge.
            (
                true,
                &[
                    (Press(Document), Document),
                    (Scope(true), Document),
                    (Scope(false), Document),
                    (Scope(true), Style),
                ],
            ),
            // 8: Style cannot be pressed with nothing in scope.
            (false, &[(Press(Style), Document)]),
            // 14: the shortcuts.
            (
                true,
                &[
                    (Press(Document), Document),
                    (ShortcutStyle, Style),
                    (ShortcutDocument, Document),
                ],
            ),
            (
                false,
                &[(ShortcutStyle, Document), (ShortcutDocument, Document)],
            ),
        ];
        for (start_non_empty, steps) in rows {
            let mut tabs = PanelTabs::new(*start_non_empty);
            let mut scope = *start_non_empty;
            let expected_start = if *start_non_empty { Style } else { Document };
            assert_eq!(tabs.active(), expected_start, "start {start_non_empty}");
            for (step, want) in *steps {
                match *step {
                    Scope(next) => {
                        scope = next;
                        tabs.observe(next);
                    }
                    Press(tab) => {
                        tabs.press(tab, scope);
                    }
                    ShortcutStyle => tabs.shortcut_style(scope),
                    ShortcutDocument => tabs.shortcut_document(),
                }
                assert_eq!(tabs.active(), *want, "start {start_non_empty}");
            }
        }
    }

    /// Criterion 4: the refused press reports it.
    #[test]
    fn pressing_style_with_an_empty_scope_is_refused() {
        let mut tabs = PanelTabs::new(false);
        assert!(!tabs.press(Style, false));
        assert!(tabs.press(Document, false));
        let mut with_objects = PanelTabs::new(true);
        assert!(with_objects.press(Style, true));
    }

    /// Criterion 1 and 4: names, order, tooltips, the dimmed tab.
    #[test]
    fn the_strip_has_two_tabs_with_tooltips_and_a_dimmed_style() {
        let empty = tab_entries(Document, false);
        let names: Vec<_> = empty.iter().map(|e| (e.name, e.label)).collect();
        assert_eq!(names, [("document", "Document"), ("style", "Style")]);
        assert_eq!(empty[0].tooltip, "Document (Shift+Ctrl+D)");
        assert_eq!(empty[1].tooltip, "Style: select an object first");
        assert_eq!(
            empty
                .iter()
                .map(|e| (e.enabled, e.selected))
                .collect::<Vec<_>>(),
            [(true, true), (false, false)]
        );
        let full = tab_entries(Style, true);
        assert_eq!(full[1].tooltip, "Style (Shift+Ctrl+F)");
        assert_eq!(
            full.iter()
                .map(|e| (e.enabled, e.selected))
                .collect::<Vec<_>>(),
            [(true, false), (true, true)]
        );
    }

    /// Criterion 4: the tooltip names the reason the tab is dimmed.
    #[test]
    fn the_dimmed_tooltip_names_the_right_reason() {
        let tip = |blocker| tab_entries_for(Document, Some(blocker))[1].tooltip;
        assert_eq!(
            tip(StyleBlocker::NothingSelected),
            "Style: select an object first"
        );
        assert_eq!(
            tip(StyleBlocker::PenTool),
            "Style: the Pen tool has nothing to style"
        );
        assert_eq!(
            tip(StyleBlocker::NodeToolWithoutPath),
            "Style: the Node tool styles paths only"
        );
        assert_eq!(
            tab_entries_for(Style, None)[1].tooltip,
            "Style (Shift+Ctrl+F)"
        );
    }

    #[test]
    fn names_round_trip() {
        assert_eq!(PanelTab::from_name("style"), Some(Style));
        assert_eq!(PanelTab::from_name("document"), Some(Document));
        assert_eq!(PanelTab::from_name("history"), None);
    }
}
