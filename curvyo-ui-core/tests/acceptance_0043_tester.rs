//! Independent tester cases for `0043-properties-tabs`, pure rule side: the
//! tab strip's entries, which tab is active after each event, and the body per
//! tab. Written from `specification.md` before the implementation was read
//! (only the public names were listed to compile against).
//!
//! Criteria: 1, 2, 4, 5, 6, 7, 8, 10, 14, 23 (the rule side of each).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    missing_docs,
    clippy::type_complexity
)]

use curvyo_ui_core::{PanelContent, PanelTab, PanelTabs, panel_body, tab_entries};
use proptest::prelude::*;

const DOC: PanelTab = PanelTab::Document;
const STYLE: PanelTab = PanelTab::Style;

// ---------------------------------------------------------------- the strip

#[test]
fn c1_two_tabs_in_the_order_document_style_with_names_and_tooltips() {
    for non_empty in [false, true] {
        for active in [DOC, STYLE] {
            let entries = tab_entries(active, non_empty);
            let names: Vec<&str> = entries.iter().map(|e| e.name).collect();
            assert_eq!(names, ["document", "style"], "no History until 0020");
            let labels: Vec<&str> = entries.iter().map(|e| e.label).collect();
            assert_eq!(labels, ["Document", "Style"]);
            assert_eq!(entries[0].tooltip, "Document (Shift+Ctrl+D)");
            if non_empty {
                assert_eq!(entries[1].tooltip, "Style (Shift+Ctrl+F)");
            } else {
                assert_eq!(entries[1].tooltip, "Style: select an object first");
            }
        }
    }
}

#[test]
fn c2_c4_exactly_one_tab_selected_and_style_is_dimmed_only_without_a_selection() {
    for non_empty in [false, true] {
        for active in [DOC, STYLE] {
            let entries = tab_entries(active, non_empty);
            assert_eq!(entries.iter().filter(|e| e.selected).count(), 1);
            assert_eq!(
                entries.iter().find(|e| e.selected).unwrap().name,
                active.name()
            );
            assert!(entries[0].enabled, "Document is never dimmed");
            assert_eq!(entries[1].enabled, non_empty);
        }
    }
}

#[test]
fn c12_tab_names_round_trip_and_unknown_names_are_none() {
    assert_eq!(PanelTab::from_name("document"), Some(DOC));
    assert_eq!(PanelTab::from_name("style"), Some(STYLE));
    for bad in [
        "", "Document", "STYLE", "history", " style", "style ", "panel", "💧",
    ] {
        assert_eq!(PanelTab::from_name(bad), None, "{bad:?}");
    }
    assert_eq!(PanelTab::from_name(DOC.name()), Some(DOC));
}

// ---------------------------------------------------------------- which tab is active

#[test]
fn c5_new_session_document_when_empty_style_when_not() {
    assert_eq!(PanelTabs::new(false).active(), DOC);
    assert_eq!(PanelTabs::new(true).active(), STYLE);
}

#[test]
fn c6_c7_edges_switch_between_document_and_style() {
    let mut t = PanelTabs::new(false);
    t.observe(true);
    assert_eq!(t.active(), STYLE, "empty -> not empty with Document");
    t.observe(false);
    assert_eq!(t.active(), DOC, "not empty -> empty with Style");
}

#[test]
fn c7_changes_that_stay_non_empty_never_switch() {
    for start in [DOC, STYLE] {
        let mut t = PanelTabs::new(true);
        assert!(t.press(start, true));
        for _ in 0..5 {
            t.observe(true);
            assert_eq!(t.active(), start);
        }
    }
    let mut t = PanelTabs::new(false);
    for _ in 0..5 {
        t.observe(false);
        assert_eq!(t.active(), DOC);
    }
}

#[test]
fn c8_a_pressed_document_survives_non_empty_changes_and_style_returns_after_empty() {
    let mut t = PanelTabs::new(true); // Style
    assert!(t.press(DOC, true));
    t.observe(true);
    t.observe(true);
    assert_eq!(
        t.active(),
        DOC,
        "sticky while the selection stays non-empty"
    );
    t.observe(false);
    assert_eq!(t.active(), DOC, "empty: Document stays");
    t.observe(true);
    assert_eq!(
        t.active(),
        STYLE,
        "empty -> non-empty: criterion 6 at the boundary"
    );
}

#[test]
fn c8_no_stale_flag_a_pressed_style_after_document_pressed_before() {
    let mut t = PanelTabs::new(true);
    assert!(t.press(DOC, true));
    assert!(t.press(STYLE, true));
    t.observe(false);
    assert_eq!(t.active(), DOC);
    t.observe(true);
    assert_eq!(t.active(), STYLE);
}

#[test]
fn c4_a_press_on_the_dimmed_style_tab_does_nothing() {
    let mut t = PanelTabs::new(false);
    assert!(!t.press(STYLE, false));
    assert_eq!(t.active(), DOC);
    // Repeated presses never change anything either.
    for _ in 0..4 {
        assert!(!t.press(STYLE, false));
        assert_eq!(t.active(), DOC);
    }
    // A press on the active Document tab is accepted and keeps it.
    assert!(t.press(DOC, false));
    assert_eq!(t.active(), DOC);
}

#[test]
fn c4_style_is_never_active_while_the_scope_is_empty() {
    let mut t = PanelTabs::new(true);
    assert_eq!(t.active(), STYLE);
    t.observe(false);
    assert_ne!(t.active(), STYLE);
    t.shortcut_style(false);
    assert_ne!(t.active(), STYLE);
}

#[test]
fn c14_shift_ctrl_f_and_shift_ctrl_d() {
    let mut t = PanelTabs::new(true);
    assert!(t.press(DOC, true));
    t.shortcut_style(true);
    assert_eq!(t.active(), STYLE, "with a selection: Style");
    t.shortcut_style(true);
    assert_eq!(t.active(), STYLE, "idempotent");

    let mut t = PanelTabs::new(false);
    t.shortcut_style(false);
    assert_eq!(t.active(), DOC, "with none: Document");

    let mut t = PanelTabs::new(true);
    t.shortcut_document();
    assert_eq!(t.active(), DOC, "Document whatever the scope");
    t.observe(true);
    assert_eq!(t.active(), DOC, "and it is as sticky as a pressed tab");
    t.shortcut_document();
    assert_eq!(t.active(), DOC);
}

// ---------------------------------------------------------------- the body

#[test]
fn c3_c10_c23_body_follows_the_tab_the_pen_and_the_scope() {
    use PanelContent::{Document, Empty, Style};
    // Document tab: its section, except while the Pen has an unfinished path.
    assert_eq!(panel_body(DOC, false, false), Document);
    assert_eq!(panel_body(DOC, false, true), Document, "criterion 24");
    assert_eq!(panel_body(DOC, true, false), Empty);
    assert_eq!(panel_body(DOC, true, true), Empty);
    // Style tab: the Style area when the scope holds objects, nothing otherwise.
    assert_eq!(panel_body(STYLE, false, true), Style);
    assert_eq!(panel_body(STYLE, false, false), Empty);
    assert_eq!(panel_body(STYLE, true, false), Empty);
}

// ---------------------------------------------------------------- properties

#[derive(Debug, Clone)]
enum Op {
    Observe(bool),
    Press(bool, bool), // (style?, scope)
    ShortcutStyle,
    ShortcutDocument,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        any::<bool>().prop_map(Op::Observe),
        (any::<bool>(), any::<bool>()).prop_map(|(s, n)| Op::Press(s, n)),
        Just(Op::ShortcutStyle),
        Just(Op::ShortcutDocument),
    ]
}

proptest! {
    /// The host's protocol: the scope is read (`observe`) before every event,
    /// then the event is applied with the same scope. Reference model written
    /// from criteria 4 to 8 and 14, with no sticky flag.
    #[test]
    fn model_random_event_sequences_match_the_spec(start in any::<bool>(), ops in prop::collection::vec(op(), 0..200)) {
        let mut t = PanelTabs::new(start);
        let (mut active, mut prev) = (if start { STYLE } else { DOC }, start);
        prop_assert_eq!(t.active(), active);
        let mut scope = start;
        for o in ops {
            // The scope the event sees: observe ops set it, others keep it.
            let (now, apply): (bool, Box<dyn Fn(&mut PanelTabs)>) = match o.clone() {
                Op::Observe(n) => (n, Box::new(|_| {})),
                Op::Press(style, n) => {
                    let tab = if style { STYLE } else { DOC };
                    (n, Box::new(move |t: &mut PanelTabs| { let _ = t.press(tab, n); }))
                }
                Op::ShortcutStyle => (scope, Box::new(move |t: &mut PanelTabs| t.shortcut_style(scope))),
                Op::ShortcutDocument => (scope, Box::new(|t: &mut PanelTabs| t.shortcut_document())),
            };
            scope = now;
            // observe first, as the session does
            t.observe(now);
            if now && !prev && active == DOC { active = STYLE; }
            if !now && prev && active == STYLE { active = DOC; }
            prev = now;
            prop_assert_eq!(t.active(), active, "after observe({}) in {:?}", now, o);
            apply(&mut t);
            match o {
                Op::Press(style, n) => {
                    if style && n { active = STYLE; }
                    if !style { active = DOC; }
                }
                Op::ShortcutStyle => { active = if scope { STYLE } else { DOC }; }
                Op::ShortcutDocument => { active = DOC; }
                Op::Observe(_) => {}
            }
            prop_assert_eq!(t.active(), active, "after {:?}", o);
            // Invariant: Style is never active with an empty scope.
            if !scope { prop_assert_eq!(t.active(), DOC); }
            // The strip agrees with the rule.
            let e = tab_entries(t.active(), scope);
            prop_assert_eq!(e.iter().filter(|x| x.selected).count(), 1);
            prop_assert_eq!(e[1].enabled, scope);
        }
    }

    /// Only an edge of the scope, a press or a shortcut changes the tab.
    #[test]
    fn model_nothing_else_changes_the_tab(start in any::<bool>(), steps in prop::collection::vec(any::<bool>(), 0..100)) {
        let mut t = PanelTabs::new(start);
        let mut prev = start;
        for n in steps {
            let before = t.active();
            t.observe(n);
            if n == prev {
                prop_assert_eq!(t.active(), before, "no edge, no switch");
            }
            prev = n;
        }
    }

    /// Body and strip stay consistent for every state: the Style body only for
    /// the Style tab with objects; never Style content under the Document tab.
    #[test]
    fn model_body_never_shows_style_under_the_document_tab(pen in any::<bool>(), objects in any::<bool>()) {
        prop_assert_ne!(panel_body(DOC, pen, objects), PanelContent::Style);
        prop_assert_ne!(panel_body(STYLE, pen, objects), PanelContent::Document);
    }
}
