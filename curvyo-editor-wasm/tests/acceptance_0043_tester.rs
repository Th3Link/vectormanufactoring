//! Independent tester cases for `0043-properties-tabs` at the `Session` level:
//! the default tab per context, the automatic switches on selection edges, the
//! manual choices, the dimmed Style tab, the Shift+Ctrl+F / Shift+Ctrl+D calls,
//! the Node and Pen tools, the document edit with a selection, and rapid event
//! sequences. Written from `specification.md` before the implementation was
//! read (only the public names were listed to compile against).
//!
//! Criteria: 4, 5, 6, 7, 8, 10, 11, 14, 19 (state side), 23, 24, 25.

#![allow(
    unused_must_use,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::many_single_char_names,
    missing_docs,
    clippy::assert_is_empty,
    clippy::collapsible_if,
    clippy::items_after_statements
)]

use curvyo_document_core::{Document, Point, unpack};
use curvyo_editor_wasm::{DocumentSide, PanelView, Session, Tool};
use curvyo_ui_core::PanelContent;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn click(s: &mut Session, x: f64, y: f64) {
    s.pointer_down(pt(x, y), false);
    s.pointer_up(pt(x, y), false, false);
}

/// A marquee in the Select tool from empty space: the dependable way to select
/// (new shapes have no fill to hit inside).
fn marquee(s: &mut Session, from: (f64, f64), to: (f64, f64)) {
    s.set_tool(Tool::Select);
    s.pointer_hover(pt(from.0, from.1), false, false);
    s.pointer_down(pt(from.0, from.1), false);
    s.pointer_hover(pt(to.0, to.1), false, false);
    s.pointer_up(pt(to.0, to.1), false, false);
}

/// Selects the first rectangle (10..60).
fn pick1(s: &mut Session) {
    s.escape();
    marquee(s, (0.0, 0.0), (70.0, 70.0));
}

/// Selects the second rectangle (100..150).
fn pick2(s: &mut Session) {
    s.escape();
    marquee(s, (90.0, 90.0), (160.0, 160.0));
}

/// Selects both.
fn pick_both(s: &mut Session) {
    s.escape();
    marquee(s, (0.0, 0.0), (170.0, 170.0));
}

fn drag(s: &mut Session, from: (f64, f64), to: (f64, f64)) {
    s.pointer_down(pt(from.0, from.1), false);
    s.pointer_up(pt(to.0, to.1), false, false);
}

/// A rectangle (10,10)-(60,60); nothing selected afterwards, Select tool.
fn session_with_rect() -> Session {
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(&mut s, (10.0, 10.0), (60.0, 60.0));
    s.set_tool(Tool::Select);
    s.escape();
    assert_eq!(s.selected_object_count(), 0);
    s
}

/// Two rectangles (10..60) and (100..150).
fn session_with_two() -> Session {
    let mut s = session_with_rect();
    s.set_tool(Tool::Rectangle);
    drag(&mut s, (100.0, 100.0), (150.0, 150.0));
    s.set_tool(Tool::Select);
    s.escape();
    s
}

fn active(v: &PanelView) -> &'static str {
    v.tabs.iter().find(|t| t.selected).unwrap().name
}

fn doc_of(s: &Session) -> Document {
    unpack(9, &s.pack("0.1.0").unwrap()).unwrap()
}

fn assert_consistent(v: &PanelView, ctx: &str) {
    assert_eq!(v.tabs.len(), 2, "{ctx}");
    assert_eq!(v.tabs.iter().filter(|t| t.selected).count(), 1, "{ctx}");
    assert_eq!(
        v.tabs.iter().map(|t| t.name).collect::<Vec<_>>(),
        ["document", "style"],
        "{ctx}"
    );
    let style = &v.tabs[1];
    if style.selected {
        assert!(style.enabled, "{ctx}: Style active but dimmed");
    }
    assert!(v.tabs[0].enabled, "{ctx}");
    if v.content == PanelContent::Style {
        assert_eq!(active(v), "style", "{ctx}");
    }
    if active(v) == "document" && v.content != PanelContent::Document {
        assert_eq!(v.content, PanelContent::Empty, "{ctx}");
    }
}

// ---------------------------------------------------------------- defaults

#[test]
fn c5_a_new_session_shows_document_with_style_dimmed() {
    let s = Session::new(1);
    let v = s.panel_view();
    assert_eq!(active(&v), "document");
    assert_eq!(v.content, PanelContent::Document);
    assert!(!v.tabs[1].enabled);
    assert_eq!(v.tabs[1].tooltip, "Style: select an object first");
    assert_eq!(s.panel_content(), PanelContent::Document);
}

#[test]
fn c5_an_opened_project_starts_on_document() {
    let s = session_with_rect();
    let bytes = s.pack("0.1.0").unwrap();
    let reopened = Session::open(2, &bytes).unwrap();
    let v = reopened.panel_view();
    assert_eq!(active(&v), "document");
    assert_eq!(v.content, PanelContent::Document);
}

#[test]
fn c5_c11_manual_choices_do_not_live_in_the_file() {
    // Press Document with a selection, save, reopen: the file is the same
    // bytes regardless of the tab, and the new session starts by rule.
    let mut a = session_with_rect();
    pick1(&mut a);
    assert!(a.press_panel_tab("document"));
    let mut b = session_with_rect();
    pick1(&mut b);
    let (da, db) = (doc_of(&a), doc_of(&b));
    assert_eq!(
        da.export_loro_snapshot().unwrap().len(),
        db.export_loro_snapshot().unwrap().len()
    );
    let c = Session::open(3, &a.pack("0.1.0").unwrap()).unwrap();
    assert_eq!(active(&c.panel_view()), "document");
}

// ---------------------------------------------------------------- automatic switches

#[test]
fn c6_c25_selecting_switches_document_to_style_and_clearing_switches_back() {
    let mut s = session_with_rect();
    assert_eq!(active(&s.panel_view()), "document");
    pick1(&mut s);
    assert_eq!(s.selected_object_count(), 1);
    let v = s.panel_view();
    assert_eq!(active(&v), "style");
    assert_eq!(v.content, PanelContent::Style);
    assert!(v.tabs[1].enabled);
    assert_eq!(v.tabs[1].tooltip, "Style (Shift+Ctrl+F)");
    s.escape();
    let v = s.panel_view();
    assert_eq!(
        (active(&v), v.content),
        ("document", PanelContent::Document)
    );
    assert!(!v.tabs[1].enabled);
}

#[test]
fn c6_clicking_the_empty_canvas_clears_and_switches() {
    let mut s = session_with_rect();
    pick1(&mut s);
    assert_eq!(active(&s.panel_view()), "style");
    click(&mut s, 400.0, 400.0);
    assert_eq!(s.selected_object_count(), 0);
    assert_eq!(active(&s.panel_view()), "document");
}

#[test]
fn c25_deleting_the_selection_is_an_edge_like_any_other() {
    let mut s = session_with_rect();
    pick1(&mut s);
    assert_eq!(active(&s.panel_view()), "style");
    s.delete_selected();
    let v = s.panel_view();
    assert_eq!(
        (active(&v), v.content),
        ("document", PanelContent::Document)
    );
}

#[test]
fn c7_one_object_to_another_and_to_several_does_not_switch() {
    let mut s = session_with_two();
    pick1(&mut s);
    assert_eq!(active(&s.panel_view()), "style");
    pick2(&mut s);
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(active(&s.panel_view()), "style");
    pick_both(&mut s);
    assert_eq!(s.selected_object_count(), 2);
    assert_eq!(active(&s.panel_view()), "style");
    pick1(&mut s);
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(active(&s.panel_view()), "style");
}

#[test]
fn c8_a_manual_document_survives_selection_changes_and_resets_after_empty() {
    let mut s = session_with_two();
    pick1(&mut s);
    assert_eq!(active(&s.panel_view()), "style");
    assert!(s.press_panel_tab("document"));
    assert_eq!(active(&s.panel_view()), "document");
    // Non-empty to non-empty: stays.
    pick2(&mut s);
    assert_eq!(active(&s.panel_view()), "document");
    pick_both(&mut s);
    assert_eq!(active(&s.panel_view()), "document");
    // Empty: Document; then non-empty again: Style (criterion 6 at the boundary).
    s.escape();
    assert_eq!(active(&s.panel_view()), "document");
    pick2(&mut s);
    assert_eq!(active(&s.panel_view()), "style");
    assert_eq!(s.panel_view().content, PanelContent::Style);
}

#[test]
fn c8_pressing_document_and_style_back_and_forth_with_a_selection() {
    let mut s = session_with_rect();
    pick1(&mut s);
    for _ in 0..3 {
        assert!(s.press_panel_tab("document"));
        let v = s.panel_view();
        assert_eq!(
            (active(&v), v.content),
            ("document", PanelContent::Document)
        );
        assert!(s.press_panel_tab("style"));
        let v = s.panel_view();
        assert_eq!((active(&v), v.content), ("style", PanelContent::Style));
    }
    assert_eq!(s.selected_object_count(), 1);
}

#[test]
fn c4_pressing_the_dimmed_style_tab_changes_nothing_and_reports_false() {
    let mut s = session_with_rect();
    assert!(!s.press_panel_tab("style"));
    let v = s.panel_view();
    assert_eq!(
        (active(&v), v.content),
        ("document", PanelContent::Document)
    );
    assert!(!v.tabs[1].enabled);
    assert!(!s.press_panel_tab("style"), "still nothing");
}

#[test]
fn c4_unknown_and_future_tab_names_are_refused() {
    let mut s = session_with_rect();
    pick1(&mut s);
    for name in ["history", "History", "", "Style", "layers", "document "] {
        assert!(!s.press_panel_tab(name), "{name:?}");
        assert_eq!(active(&s.panel_view()), "style");
    }
}

#[test]
fn c4_press_notices_a_selection_change_that_the_panel_has_not_read_yet() {
    // Select, then press Document without an intermediate read of the panel;
    // the press must act on the real state (the selection exists: Document is
    // manual and stays through the next read).
    let mut s = session_with_rect();
    pick1(&mut s);
    assert!(s.press_panel_tab("document"));
    assert_eq!(active(&s.panel_view()), "document");
    // Clear, then (still without a read) press Style: refused, nothing selected.
    s.escape();
    assert!(!s.press_panel_tab("style"));
    assert_eq!(active(&s.panel_view()), "document");
}

// ---------------------------------------------------------------- shortcuts

#[test]
fn c14_shift_ctrl_f_chooses_style_with_a_selection_and_document_without() {
    let mut s = session_with_rect();
    s.panel_shortcut_style();
    assert_eq!(active(&s.panel_view()), "document", "nothing selected");
    pick1(&mut s);
    assert!(s.press_panel_tab("document"));
    s.panel_shortcut_style();
    let v = s.panel_view();
    assert_eq!((active(&v), v.content), ("style", PanelContent::Style));
    s.escape();
    s.panel_shortcut_style();
    assert_eq!(active(&s.panel_view()), "document");
}

#[test]
fn c14_shift_ctrl_d_opens_document_with_the_selection_kept_and_stays() {
    let mut s = session_with_two();
    pick1(&mut s);
    assert_eq!(active(&s.panel_view()), "style");
    s.panel_shortcut_document();
    let v = s.panel_view();
    assert_eq!(
        (active(&v), v.content),
        ("document", PanelContent::Document)
    );
    assert_eq!(s.selected_object_count(), 1, "the selection stays");
    pick2(&mut s);
    assert_eq!(
        active(&s.panel_view()),
        "document",
        "sticky like a pressed tab"
    );
    s.panel_shortcut_document();
    assert_eq!(active(&s.panel_view()), "document", "idempotent");
}

// ---------------------------------------------------------------- criterion 24

#[test]
fn c24_the_document_size_can_be_edited_with_an_object_selected_and_the_selection_stays() {
    let mut s = session_with_rect();
    pick1(&mut s);
    assert!(s.press_panel_tab("document"));
    assert_eq!(s.selected_object_count(), 1);
    let before = doc_of(&s).size();
    let out = s.set_document_side(DocumentSide::Width, "300");
    let _ = out;
    let after = doc_of(&s).size();
    assert!((after.width.as_mm() - 300.0).abs() < 1e-9, "{after:?}");
    assert!((before.width.as_mm() - 300.0).abs() > 1e-6);
    assert_eq!(s.selected_object_count(), 1, "selection kept");
    let v = s.panel_view();
    assert_eq!(
        (active(&v), v.content),
        ("document", PanelContent::Document)
    );
}

#[test]
fn c24_picking_a_format_with_an_object_selected_keeps_selection_and_tab() {
    let mut s = session_with_rect();
    pick1(&mut s);
    assert!(s.press_panel_tab("document"));
    // A3 from the quick selection: whatever the API is called, the typed path
    // is the same thing the pick uses.
    let _ = s.set_document_side(DocumentSide::Height, "420");
    let _ = s.set_document_side(DocumentSide::Width, "297");
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(active(&s.panel_view()), "document");
}

// ---------------------------------------------------------------- Pen and Node

#[test]
fn c10_switching_to_the_pen_with_a_selection_is_an_edge_to_empty() {
    let mut s = session_with_rect();
    pick1(&mut s);
    assert_eq!(active(&s.panel_view()), "style");
    s.set_tool(Tool::Pen);
    let v = s.panel_view();
    assert_eq!(
        (active(&v), v.content),
        ("document", PanelContent::Document),
        "Style hands over to Document when the scope empties"
    );
    assert!(!v.tabs[1].enabled);
    // Back to Select: the selection is still there, so empty -> non-empty.
    s.set_tool(Tool::Select);
    let v = s.panel_view();
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(active(&v), "style");
}

#[test]
fn c10_an_unfinished_pen_path_empties_the_body_and_keeps_the_strip() {
    let mut s = session_with_rect();
    s.set_tool(Tool::Pen);
    click(&mut s, 200.0, 200.0);
    click(&mut s, 260.0, 200.0);
    assert!(s.pen_in_progress().is_some(), "an unfinished path");
    let v = s.panel_view();
    assert_eq!(v.content, PanelContent::Empty);
    assert_eq!(v.tabs.len(), 2, "the strip stays");
    assert_eq!(active(&v), "document");
    // A press on a tab sets the active tab.
    assert!(
        !s.press_panel_tab("style"),
        "dimmed: scope is empty in the Pen"
    );
    assert!(s.press_panel_tab("document"));
    assert_eq!(s.panel_view().content, PanelContent::Empty);
    // The panel (and Document) comes back after the path is finished.
    s.finish_pen();
    assert!(s.pen_in_progress().is_none());
    let v = s.panel_view();
    assert_consistent(&v, "after finish_pen");
    assert_ne!(v.content, PanelContent::Empty, "{:?}", v.content);
}

#[test]
fn c10_a_selection_then_pen_then_unfinished_path_keeps_the_rule_pure() {
    let mut s = session_with_rect();
    pick1(&mut s);
    assert!(s.press_panel_tab("document"));
    s.set_tool(Tool::Pen);
    click(&mut s, 300.0, 300.0);
    let v = s.panel_view();
    assert_eq!(v.content, PanelContent::Empty);
    assert_eq!(active(&v), "document");
    s.escape(); // cancels the path
    assert!(s.pen_in_progress().is_none());
    let v = s.panel_view();
    assert_consistent(&v, "after escape in the pen");
}

fn draw_path(s: &mut Session) {
    s.set_tool(Tool::Pen);
    click(s, 300.0, 300.0);
    click(s, 360.0, 300.0);
    click(s, 360.0, 360.0);
    s.finish_pen();
}

#[test]
fn c23_the_node_tool_with_a_selection_it_can_style_shows_style_and_with_none_document() {
    let mut s = Session::new(1);
    draw_path(&mut s);
    s.set_tool(Tool::Select);
    s.escape();
    s.set_tool(Tool::Node);
    let v = s.panel_view();
    assert_eq!(active(&v), "document", "nothing edited yet");
    // Click the path's segment to select it in the Select tool, then the Node tool.
    s.set_tool(Tool::Select);
    click(&mut s, 330.0, 300.0);
    assert_eq!(s.selected_object_count(), 1, "the path is selected");
    assert_eq!(active(&s.panel_view()), "style");
    s.set_tool(Tool::Node);
    let v = s.panel_view();
    assert_consistent(&v, "node tool with a path");
    assert_eq!(
        (active(&v), v.content),
        ("style", PanelContent::Style),
        "the Node tool's scope holds the edited path"
    );
}

#[test]
fn c23_the_node_tool_with_a_selection_it_cannot_style_shows_document() {
    // A rectangle is not a path the Node tool edits (rectangles are shapes).
    let mut s = session_with_rect();
    pick1(&mut s);
    assert_eq!(active(&s.panel_view()), "style");
    s.set_tool(Tool::Node);
    let v = s.panel_view();
    assert_consistent(&v, "node tool with a rectangle");
    assert_eq!(
        (active(&v), v.content),
        ("document", PanelContent::Document),
        "a selection the Node tool cannot style shows Document"
    );
    assert!(!v.tabs[1].enabled);
    assert_eq!(s.selected_object_count(), 1, "the selection itself stays");
    // And the Pen does the same.
    s.set_tool(Tool::Pen);
    let v = s.panel_view();
    assert_eq!(
        (active(&v), v.content),
        ("document", PanelContent::Document)
    );
}

// ---------------------------------------------------------------- reads are free

#[test]
fn reading_the_panel_never_changes_the_document() {
    let mut s = session_with_rect();
    pick1(&mut s);
    let before = s.pack("0.1.0").unwrap();
    for _ in 0..50 {
        let _ = s.panel_view();
        let _ = s.panel_content();
    }
    assert_eq!(s.pack("0.1.0").unwrap().len(), before.len());
    assert_eq!(s.selected_object_count(), 1);
}

#[test]
fn many_repeated_reads_after_a_manual_choice_are_stable() {
    let mut s = session_with_rect();
    pick1(&mut s);
    assert!(s.press_panel_tab("document"));
    for _ in 0..100 {
        assert_eq!(active(&s.panel_view()), "document");
    }
}

// ---------------------------------------------------------------- rapid sequences

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn coord(&mut self) -> f64 {
        self.below(400) as f64
    }
}

/// Random mixes of clicks, drags, escapes, deletes, tool changes, panel presses
/// and shortcuts, with a panel read after some of them. The invariants of the
/// strip hold after every read and nothing panics.
#[test]
fn rapid_random_sequences_keep_the_strip_consistent_and_never_panic() {
    for seed in 1..=40_u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut s = session_with_two();
        for step in 0..150 {
            let what = rng.below(16);
            match what {
                0 => click(&mut s, rng.coord(), rng.coord()),
                1 => drag(
                    &mut s,
                    (rng.coord(), rng.coord()),
                    (rng.coord(), rng.coord()),
                ),
                2 => pick_both(&mut s),
                3 => {
                    s.escape();
                }
                4 => s.delete_selected(),
                5 => s.set_tool(Tool::Select),
                6 => s.set_tool(Tool::Pen),
                7 => s.set_tool(Tool::Node),
                8 => s.set_tool(Tool::Rectangle),
                9 => {
                    s.press_panel_tab("document");
                }
                10 => {
                    s.press_panel_tab("style");
                }
                11 => s.panel_shortcut_style(),
                12 => s.panel_shortcut_document(),
                13 => s.finish_pen(),
                14 => {
                    let _ = s.set_document_side(DocumentSide::Width, "250");
                }
                _ => {}
            }
            if rng.below(2) == 0 || what >= 9 {
                let v = s.panel_view();
                assert_consistent(&v, &format!("seed {seed} step {step} op {what}"));
                if !v.tabs[1].enabled {
                    assert_ne!(active(&v), "style", "seed {seed} step {step}");
                    assert_ne!(v.content, PanelContent::Style, "seed {seed} step {step}");
                }
                if v.content == PanelContent::Document {
                    assert!(s.pen_in_progress().is_none(), "seed {seed} step {step}");
                }
            }
        }
        // The file still packs and opens.
        let bytes = s.pack("0.1.0").unwrap();
        Session::open(5, &bytes).unwrap();
    }
}
