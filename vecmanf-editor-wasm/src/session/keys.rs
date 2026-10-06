//! `Session`'s keyboard: the one gate and key table, the Escape cascade and
//! Delete (`specs/edit-interaction-polish/`, Parts D and F; `adrs.md`
//! decisions 4 and 6).

use vecmanf_ui_core::{EntryKey, KeyEntryRefusal};

use super::{Session, Tool};

/// One key event as the frontend reports it: the DOM `key` value, the
/// modifiers (`ctrl` already folds in Cmd) and the two facts only the DOM
/// knows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // one flag per DOM fact, as the host reports them
pub struct KeyInput<'a> {
    /// `KeyboardEvent.key`: `"r"`, `"*"`, `"Delete"`, `"Escape"` and so on.
    pub key: &'a str,
    /// Shift is down.
    pub shift: bool,
    /// Ctrl or Cmd is down.
    pub ctrl: bool,
    /// Alt is down.
    pub alt: bool,
    /// The event is an auto-repeat of a held key.
    pub repeat: bool,
    /// Focus is in a text field, select, button, switch or contenteditable
    /// element, an IME composition is running, or Space is held: knowledge
    /// only the DOM has.
    pub dom_blocked: bool,
}

/// The one-line message of a key that cannot act (criterion 59); the
/// frontend owns the wording and shows it for 2 s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyHint {
    /// Several objects are selected: "Select one object to type a value".
    SelectOne,
}

/// What `Session::key_down` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// The gate or the key table said no: nothing happened, and the page's
    /// default for the key is not to be prevented.
    Ignored,
    /// The active tool changed.
    ToolChanged,
    /// A typed entry opened.
    EntryOpened,
    /// Delete or Backspace ran (it may have found nothing to delete).
    Deleted,
    /// Escape took this step.
    Escape(EscapeStep),
    /// Enter finished the Pen path.
    PenFinished,
    /// The key could not act; show this hint.
    Hint(KeyHint),
}

impl KeyOutcome {
    /// The string the frontend reads (ADR 0001 §5: strings and scalars across
    /// the wasm boundary). Everything but `"ignored"` means the key was
    /// handled and its page default is to be prevented.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ignored => "ignored",
            Self::ToolChanged => "tool",
            Self::EntryOpened => "entry",
            Self::Deleted => "deleted",
            Self::PenFinished => "pen",
            Self::Escape(EscapeStep::ClosedEntry) => "escape-entry",
            Self::Escape(EscapeStep::CancelledDrag) => "escape-drag",
            Self::Escape(EscapeStep::ClearedState) => "escape-state",
            Self::Escape(EscapeStep::LeftTool) => "escape-tool",
            Self::Escape(EscapeStep::Nothing) => "escape-none",
            Self::Hint(KeyHint::SelectOne) => "hint-select-one",
        }
    }
}

/// The one step a single Escape press took (criterion 42); the document is
/// never written by any of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeStep {
    /// An open entry chip was closed.
    ClosedEntry,
    /// A drag in flight was cancelled.
    CancelledDrag,
    /// The active tool's own state was cleared: the Pen path, the Node
    /// tool's node and segment selection, the Select tool's object
    /// selection.
    ClearedState,
    /// The tool was left for the Select tool.
    LeftTool,
    /// There was nothing to do.
    Nothing,
}

/// How many objects the Select tool has selected, as far as the key table
/// cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selected {
    None,
    One,
    Many,
}

/// Everything about the session the key table reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyState {
    tool: Tool,
    /// A drag is in flight in the active tool, or the canvas is panned.
    operation_in_flight: bool,
    /// The Pen has an unfinished path.
    pen_open: bool,
    /// An entry chip is open.
    entry_open: bool,
    selected: Selected,
}

/// What the key table decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyAction {
    Ignore,
    SetTool(Tool),
    OpenEntry(EntryKey),
    Delete,
    FinishPen,
    Escape,
    Hint(KeyHint),
}

/// The key table and its gate (criteria 54, 55, 60, 61) as a pure function:
/// the one place that says what a key does in which state.
fn decide(input: &KeyInput<'_>, state: KeyState) -> KeyAction {
    match input.key {
        // Never gated by the list of criterion 55; a held Escape acts once.
        "Escape" => {
            return if input.repeat {
                KeyAction::Ignore
            } else {
                KeyAction::Escape
            };
        }
        "Enter" => {
            return if state.tool == Tool::Pen && state.pen_open {
                KeyAction::FinishPen
            } else {
                KeyAction::Ignore
            };
        }
        _ => {}
    }
    if input.repeat || input.ctrl || input.alt || input.dom_blocked {
        return KeyAction::Ignore;
    }
    // Letters are matched without regard to Caps Lock.
    let letter = match input.key.chars().collect::<Vec<_>>().as_slice() {
        [c] if c.is_ascii_alphabetic() => Some(c.to_ascii_lowercase()),
        _ => None,
    };
    let binding = match (letter, input.key) {
        (Some('b'), _) => Binding::Tool(Tool::Pen),
        (Some('n'), _) => Binding::Tool(Tool::Node),
        (Some('e'), _) => Binding::Tool(Tool::Ellipse),
        (_, "*") => Binding::Tool(Tool::PolygonStar),
        (Some('r'), _) => Binding::Entry(EntryKey::Angle, Tool::Rectangle),
        (Some('s'), _) => Binding::Entry(EntryKey::Size, Tool::Select),
        (_, "Delete" | "Backspace") => Binding::Delete,
        _ => return KeyAction::Ignore,
    };
    // Shift is allowed only for `*`.
    if input.shift && input.key != "*" {
        return KeyAction::Ignore;
    }
    if state.operation_in_flight || state.pen_open || state.entry_open {
        return KeyAction::Ignore;
    }
    match binding {
        Binding::Tool(tool) => KeyAction::SetTool(tool),
        Binding::Delete => KeyAction::Delete,
        // The one state that changes a letter: the Select tool with an
        // object selected acts on it; every other state switches tool.
        Binding::Entry(entry, tool) => match (state.tool, state.selected) {
            (Tool::Select, Selected::One) => KeyAction::OpenEntry(entry),
            (Tool::Select, Selected::Many) => KeyAction::Hint(KeyHint::SelectOne),
            _ => KeyAction::SetTool(tool),
        },
    }
}

/// What a key that passed the gate is bound to.
#[derive(Debug, Clone, Copy)]
enum Binding {
    Tool(Tool),
    /// The typed entry in the Select tool with a selection, else the tool.
    Entry(EntryKey, Tool),
    Delete,
}

impl Session {
    /// The selection as the key table sees it: only the Select tool's
    /// object selection counts.
    fn selected_for_keys(&self) -> Selected {
        if self.tool != Tool::Select {
            return Selected::None;
        }
        match self.selection.ids().len() {
            0 => Selected::None,
            1 => Selected::One,
            _ => Selected::Many,
        }
    }

    /// Whether a drag is in flight in the active tool (move, resize, rotate,
    /// skew, parameter handle, create-drag, node or handle drag), or the
    /// canvas is being panned (criterion 55). The Pen's unfinished path is a
    /// separate fact.
    fn operation_in_flight(&self) -> bool {
        let in_tool = match self.tool {
            Tool::Select => self.select.drag_in_flight(),
            Tool::Node => self.node.drag_in_flight(),
            Tool::Rectangle => self.rectangle.drag_in_flight(),
            Tool::Ellipse => self.ellipse.drag_in_flight(),
            Tool::PolygonStar => self.poly_star.drag_in_flight(),
            Tool::Pen => false,
        };
        in_tool || self.is_panning()
    }

    fn pen_path_open(&self) -> bool {
        self.pen.in_progress_nodes().is_some()
    }

    /// The number of selected objects.
    #[must_use]
    pub fn selected_object_count(&self) -> usize {
        self.selection.ids().len()
    }

    /// One key press: the gate of criterion 55, then the key table of
    /// criterion 54. The frontend forwards the key and what the DOM alone
    /// knows and prevents the page's default for every outcome except
    /// [`KeyOutcome::Ignored`], so Ctrl+R and Ctrl+S keep their page and menu
    /// behaviour.
    pub fn key_down(&mut self, input: KeyInput<'_>) -> KeyOutcome {
        let state = KeyState {
            tool: self.tool,
            operation_in_flight: self.operation_in_flight(),
            pen_open: self.pen_path_open(),
            entry_open: self.select.has_entry(),
            selected: self.selected_for_keys(),
        };
        match decide(&input, state) {
            KeyAction::Ignore => KeyOutcome::Ignored,
            KeyAction::SetTool(tool) => {
                self.set_tool(tool);
                KeyOutcome::ToolChanged
            }
            KeyAction::OpenEntry(key) => {
                let objects = self.objects();
                match self
                    .select
                    .open_entry_for_key(&objects, &self.selection, key)
                {
                    Ok(()) => KeyOutcome::EntryOpened,
                    Err(KeyEntryRefusal::SeveralSelected) => KeyOutcome::Hint(KeyHint::SelectOne),
                    Err(KeyEntryRefusal::NothingSelected) => KeyOutcome::Ignored,
                }
            }
            KeyAction::Delete => {
                self.delete_selected();
                KeyOutcome::Deleted
            }
            KeyAction::FinishPen => {
                self.finish_pen();
                KeyOutcome::PenFinished
            }
            KeyAction::Escape => KeyOutcome::Escape(self.escape()),
            KeyAction::Hint(hint) => KeyOutcome::Hint(hint),
        }
    }

    /// Cancels the drag in flight in the active tool, writing nothing: the
    /// shared step 2 of the Escape cascade, a lost pointer and a non-finite
    /// release. Returns whether there was one. The Pen has no separate drag:
    /// its step 2 and 3 are one, see [`Session::escape`].
    pub(super) fn cancel_drag_in_flight(&mut self) -> bool {
        match self.tool {
            Tool::Select => {
                let was = self.select.drag_in_flight();
                if was {
                    self.select.escape();
                }
                was
            }
            Tool::Node => self.node.cancel_drag(),
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => self.shape_escape(),
            Tool::Pen => false,
        }
    }

    /// Escape: exactly one step per press, the first that applies
    /// (`specs/edit-interaction-polish/` criterion 42), and the document is
    /// never written:
    ///
    /// 1. an open entry chip closes;
    /// 2. a drag in flight is cancelled and the selection stays as it was;
    /// 3. the tool's own state is cleared: the Pen path (also during a handle
    ///    drag, steps 2 and 3 in one), the Node tool's node and segment
    ///    selection, the Select tool's object selection;
    /// 4. a tool other than Select is left for the Select tool, with the
    ///    object selection exactly as it was.
    ///
    /// With the pointer button still down steps 3 and 4 are skipped: only the
    /// drag is cancelled (criterion 49).
    pub fn escape(&mut self) -> EscapeStep {
        if self.select.has_entry() {
            self.select.cancel_entry();
            return EscapeStep::ClosedEntry;
        }
        if self.cancel_drag_in_flight() {
            return EscapeStep::CancelledDrag;
        }
        if self.tool == Tool::Pen && self.pen.escape() {
            // The Pen's step 2 and 3 are one: the unfinished path goes,
            // also during a handle drag.
            return if self.button_down {
                EscapeStep::CancelledDrag
            } else {
                EscapeStep::ClearedState
            };
        }
        if self.button_down {
            return EscapeStep::Nothing;
        }
        let cleared = match self.tool {
            Tool::Pen | Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => false,
            Tool::Node => self.node.clear_selection(),
            Tool::Select => {
                let had = !self.selection.is_empty();
                self.selection.clear();
                had
            }
        };
        if cleared {
            return EscapeStep::ClearedState;
        }
        if self.tool == Tool::Select {
            return EscapeStep::Nothing;
        }
        self.set_tool(Tool::Select);
        EscapeStep::LeftTool
    }

    /// The browser took the pointer away (a system gesture, an alert) or the
    /// window lost focus: cancels any drag in flight and forgets the button.
    pub fn pointer_cancelled(&mut self) {
        self.cancel_gesture();
        self.button_down = false;
    }

    /// Cancels whatever gesture the active tool has in flight when the
    /// pointer is lost: a drag, or, in the Pen, the unfinished path (the Pen
    /// has no way to cancel only a handle drag).
    pub(super) fn cancel_gesture(&mut self) {
        if !self.cancel_drag_in_flight() && self.tool == Tool::Pen {
            self.pen.escape();
        }
    }

    /// Acceptance criteria 19, 21 (Delete/Backspace, or the contextual
    /// toolbar's Delete button): removes every selected object when the
    /// Select or Node tool is active. A no-op for every other tool.
    pub fn delete_selected(&mut self) {
        self.flush_select_bar_preview();
        match self.tool {
            Tool::Select => {
                let objects = self.objects();
                self.select
                    .delete_selected(&self.document, &objects, &mut self.selection);
            }
            Tool::Node => self.node.delete_selected(&self.document),
            Tool::Pen | Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOLS: [Tool; 6] = [
        Tool::Select,
        Tool::Pen,
        Tool::Node,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
    ];
    /// One gate condition, applied to a key and a state.
    type Blocker = Box<dyn Fn(&mut KeyInput<'_>, &mut KeyState)>;

    const SELECTIONS: [Selected; 3] = [Selected::None, Selected::One, Selected::Many];

    fn state(tool: Tool, selected: Selected) -> KeyState {
        KeyState {
            tool,
            operation_in_flight: false,
            pen_open: false,
            entry_open: false,
            selected,
        }
    }

    fn key(key: &str) -> KeyInput<'_> {
        KeyInput {
            key,
            ..KeyInput::default()
        }
    }

    /// Criterion 54: B, N, E and `*` select their tool in every state.
    #[test]
    fn b_n_e_and_star_select_their_tool_in_every_state() {
        for tool in TOOLS {
            for selected in SELECTIONS {
                for (k, target) in [
                    ("b", Tool::Pen),
                    ("n", Tool::Node),
                    ("e", Tool::Ellipse),
                    ("*", Tool::PolygonStar),
                ] {
                    assert_eq!(
                        decide(&key(k), state(tool, selected)),
                        KeyAction::SetTool(target),
                        "{k} in {tool:?} with {selected:?}"
                    );
                }
            }
        }
    }

    /// Criterion 54: R and S act on the selection only in the Select tool
    /// with an object selected; every other state switches tool; several
    /// objects give the hint and change nothing.
    #[test]
    fn r_and_s_act_on_the_selection_only_in_the_select_tool() {
        for tool in TOOLS {
            for selected in SELECTIONS {
                for (k, entry, target) in [
                    ("r", EntryKey::Angle, Tool::Rectangle),
                    ("s", EntryKey::Size, Tool::Select),
                ] {
                    let expected = match (tool, selected) {
                        (Tool::Select, Selected::One) => KeyAction::OpenEntry(entry),
                        (Tool::Select, Selected::Many) => KeyAction::Hint(KeyHint::SelectOne),
                        _ => KeyAction::SetTool(target),
                    };
                    assert_eq!(
                        decide(&key(k), state(tool, selected)),
                        expected,
                        "{k} in {tool:?} with {selected:?}"
                    );
                }
            }
        }
    }

    /// Letters are matched without regard to Caps Lock.
    #[test]
    fn letters_are_case_insensitive() {
        for (upper, lower) in [("B", "b"), ("N", "n"), ("E", "e"), ("R", "r"), ("S", "s")] {
            assert_eq!(
                decide(&key(upper), state(Tool::Select, Selected::One)),
                decide(&key(lower), state(Tool::Select, Selected::One)),
                "{upper}"
            );
        }
    }

    /// Criterion 61: Delete and Backspace pass the same gate; what they
    /// delete is the tool's own business.
    #[test]
    fn delete_and_backspace_are_one_action_in_every_tool() {
        for tool in TOOLS {
            for k in ["Delete", "Backspace"] {
                assert_eq!(
                    decide(&key(k), state(tool, Selected::One)),
                    KeyAction::Delete
                );
            }
        }
    }

    /// Keys that are not bound do nothing; M and K have no entry yet.
    #[test]
    fn unbound_keys_do_nothing() {
        for k in [
            "m",
            "M",
            "k",
            "K",
            "x",
            "1",
            "ArrowLeft",
            "Tab",
            "F5",
            " ",
            "Shift",
        ] {
            assert_eq!(
                decide(&key(k), state(Tool::Select, Selected::One)),
                KeyAction::Ignore,
                "{k}"
            );
        }
    }

    /// The gate (criterion 55), one condition at a time and in pairs, for
    /// every key it covers in every tool: no effect at all.
    #[test]
    fn every_gate_condition_blocks_every_gated_key() {
        let gated = ["b", "n", "e", "r", "s", "*", "Delete", "Backspace"];
        let mut blockers: Vec<(&str, Blocker)> = vec![
            ("ctrl", Box::new(|i, _| i.ctrl = true)),
            ("alt", Box::new(|i, _| i.alt = true)),
            ("repeat", Box::new(|i, _| i.repeat = true)),
            ("dom", Box::new(|i, _| i.dom_blocked = true)),
            ("drag", Box::new(|_, s| s.operation_in_flight = true)),
            ("pen path", Box::new(|_, s| s.pen_open = true)),
            ("entry", Box::new(|_, s| s.entry_open = true)),
        ];
        // Shift blocks everything but `*`.
        blockers.push(("shift", Box::new(|i, _| i.shift = true)));
        for tool in TOOLS {
            for selected in SELECTIONS {
                for k in gated {
                    for (a, (name_a, block_a)) in blockers.iter().enumerate() {
                        for (b, (name_b, block_b)) in blockers.iter().enumerate() {
                            if b < a {
                                continue;
                            }
                            let mut input = key(k);
                            let mut st = state(tool, selected);
                            block_a(&mut input, &mut st);
                            block_b(&mut input, &mut st);
                            // `*` with only Shift is the one allowed pair.
                            let only_shift_on_star =
                                k == "*" && *name_a == "shift" && *name_b == "shift";
                            if only_shift_on_star {
                                continue;
                            }
                            assert_eq!(
                                decide(&input, st),
                                KeyAction::Ignore,
                                "{k} in {tool:?} with {selected:?} blocked by {name_a} + {name_b}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Shift is allowed for `*` (it is Shift+8 on many layouts).
    #[test]
    fn shift_is_allowed_for_star_only() {
        let st = state(Tool::Select, Selected::One);
        let shifted = |k| KeyInput {
            key: k,
            shift: true,
            ..KeyInput::default()
        };
        assert_eq!(
            decide(&shifted("*"), st),
            KeyAction::SetTool(Tool::PolygonStar)
        );
        for k in ["B", "N", "E", "R", "S", "Delete"] {
            assert_eq!(decide(&shifted(k), st), KeyAction::Ignore, "Shift+{k}");
        }
    }

    /// Criterion 55: Ctrl+R, Ctrl+S, Cmd+R (the frontend folds Cmd into
    /// `ctrl`) and Alt+E change no tool.
    #[test]
    fn modified_letters_are_ignored() {
        let st = state(Tool::Select, Selected::None);
        for (k, ctrl, alt) in [("r", true, false), ("s", true, false), ("e", false, true)] {
            let input = KeyInput {
                key: k,
                ctrl,
                alt,
                ..KeyInput::default()
            };
            assert_eq!(decide(&input, st), KeyAction::Ignore, "{k}");
        }
    }

    /// Escape, Enter, Space and the modifier keys are never gated by the
    /// list of criterion 55; Escape alone is gated by repeat (criterion 60).
    #[test]
    fn escape_and_enter_are_not_gated_by_the_list() {
        let mut busy = state(Tool::Pen, Selected::None);
        busy.operation_in_flight = true;
        busy.pen_open = true;
        busy.entry_open = true;
        let escape = KeyInput {
            key: "Escape",
            ctrl: true,
            alt: true,
            dom_blocked: true,
            ..KeyInput::default()
        };
        assert_eq!(decide(&escape, busy), KeyAction::Escape);
        assert_eq!(
            decide(
                &KeyInput {
                    repeat: true,
                    ..escape
                },
                busy
            ),
            KeyAction::Ignore,
            "criterion 60: a held Escape acts once"
        );
        assert_eq!(decide(&key("Enter"), busy), KeyAction::FinishPen);
        assert_eq!(
            decide(&key("Enter"), state(Tool::Pen, Selected::None)),
            KeyAction::Ignore,
            "no path to finish"
        );
        let mut select_with_path = state(Tool::Select, Selected::One);
        select_with_path.pen_open = true;
        assert_eq!(decide(&key("Enter"), select_with_path), KeyAction::Ignore);
    }

    /// The codes the frontend reads are distinct and only `ignored` means
    /// "do not prevent the default".
    #[test]
    fn outcome_codes_are_distinct() {
        let all = [
            KeyOutcome::Ignored,
            KeyOutcome::ToolChanged,
            KeyOutcome::EntryOpened,
            KeyOutcome::Deleted,
            KeyOutcome::PenFinished,
            KeyOutcome::Escape(EscapeStep::ClosedEntry),
            KeyOutcome::Escape(EscapeStep::CancelledDrag),
            KeyOutcome::Escape(EscapeStep::ClearedState),
            KeyOutcome::Escape(EscapeStep::LeftTool),
            KeyOutcome::Escape(EscapeStep::Nothing),
            KeyOutcome::Hint(KeyHint::SelectOne),
        ];
        let mut codes: Vec<&str> = all.iter().map(|o| o.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all.len());
        assert_eq!(KeyOutcome::Ignored.code(), "ignored");
    }
}
