# Editing quick wins: select all and keyboard nudge

Status: Ready (the criteria are complete and testable; `adrs.md` and the UX notes exist). Build after PR #80 (`story/multi-object-transform`) merges: it needs #80's group box, multi-object move and key table. It is a Proposal that the lead scheduled because it is small and reversible in one PR; the customer can veto it at the demo.
Priority: Should
Origin: Proposal (collected from the ux-engineer's and the tester's reviews; both keys are listed as "Reserved, not built" in `docs/design-system.md`, "Keyboard concept", and the nudge as "needs a decision (scope growth)" in the shortcut table). Not accepted until the customer says so; small and reversible in one PR.

## User value

As a maker I want Ctrl+A to select every object and the arrow keys to move the selection by a small, exact amount, so that I do not drag a marquee over the whole page and do not fight the mouse for a 1 mm correction.

**Reference tools.** Inkscape: Ctrl+A selects all objects in the current layer; arrow keys move the selection by 2 px (a preference), Shift+Arrow by 20 px, Alt+Arrow by one screen pixel. Illustrator: Ctrl+A, arrow keys by 1 pt (a preference), Shift+Arrow by 10x. LightBurn: Ctrl+A and arrow nudge by a set distance. We take 1 mm and 10 mm, document units, independent of zoom, and leave Alt out: Alt+drag and Alt+arrow belong to the window manager on several Linux desktops (`0014` plan notes).

## Acceptance criteria

### Select all

1. Given the Select tool, the canvas focused (not a text field, select or contenteditable), no drag in flight, no chip open, when the maker presses Ctrl+A (Cmd+A on macOS), then every object of the current context is selected and nothing else changes. "Context" is the document root, or the entered group once `0023` exists (its direct children, as marquee selection does). Hidden and locked objects (`0039`, when layers exist) are not selected; until then every object is. Test: 5 objects, Ctrl+A, the selection is those 5, in stacking order.
2. Given Ctrl+A with objects already selected, then the selection becomes all objects (it replaces, it does not toggle). Given a document with no object, then nothing changes and no hint appears, but the key is still consumed (`preventDefault`), so the page's own select-all does not highlight the interface text.
3. Given Ctrl+A in any tool but Select (Pen, shape tools, Node), then the key is ignored without `preventDefault`. (Node-tool select-all of nodes is out of scope.) Given focus in a text field, then the field's own select-all runs and the document selection does not change.
4. Given the result, then the selection box, the subject line ("5 objects") and every other selection-dependent surface show in the frame of the key; the panel's active tab follows `0043` (a selection that becomes non-empty makes Style active). For a screen reader a visually hidden live region says "Selected 5 objects." Select all is view state: it creates no step and nothing is written (ADR 0009 §2).
5. Given 5,000 objects, then Ctrl+A is drawn within 100 ms (release build, desktop).
6. Given Escape, then it clears the selection as today; there is no Ctrl+Shift+A (Inkscape has none for Deselect either, Escape does it).

### Nudge

7. Given the Select tool, one or more objects selected, the canvas focused, no drag in flight, no chip open, no text field focused and the Pen idle, when the maker presses an arrow key, then the selection moves **1 mm** in that direction in document coordinates (Right is +x, Down is +y, Y grows downward, ADR 0002 §4); with Shift held, **10 mm**. Alt is not used, and an arrow key with Ctrl, Cmd or Alt down does nothing (no `preventDefault`). The distance does not depend on zoom or on the display unit (the unit is a display setting, `0015`).
8. Given a nudge, then it moves the objects exactly like a typed relative move (`M`): the same operation, the same commit label `translate_objects` (operation name "Move" in `0020`), the same rules for compound paths, primitives, paths, groups (a group moves as one, `0023`) and several objects (`0019`, one commit for all). Nothing is scaled or rotated and no stroke width changes.
9. Given the key held, then each auto-repeat event moves once more (unlike the letter shortcuts, which ignore repeat), the objects and the selection box move in the frame of each event, and the whole held press, from the first key-down to the key-up, is **one step** in the history. Every event writes its commit at once (the document is always true: Save, a merge or a tool change at any moment sees the moved objects), and each repeat event joins the run's step as a **continuation** (ADR 0014 §2: the commit repeats the previous step's header `translate_objects;s=<seq>`). A continuation is allowed only for an event that is a repeat, with the same arrow and the same Shift state as the previous nudge, at most 600 ms after it, and only while the document version is still that of the run's last commit; if anything was committed or merged in between (a peer's update, another edit), the event opens a new step. While a step runs, a **move readout** next to the selection box shows the step's distance in millimetres whatever the display unit ("Δ 3.0, 0.0 mm", X right, Y down, one decimal, real minus sign); it appears with the first move of a step, ignores the pointer, is hidden from assistive technology and goes 800 ms after the last move. After the step ends a visually hidden live region says "Moved 11 mm right." or "Moved 3 mm right, 1 mm down." A run that ends when no repeat event arrives for 600 ms ends the step as well (the key-up can be missed on some platforms, `docs/design-system.md` notes for Shift on WebKitGTK). A new key-down after the key-up is a new step. A different arrow key or Shift pressed or released during the run ends the step and starts a new one. Test: 1 press = 1 step of 1 mm; 1 press with 10 repeat events = 1 step of 11 mm; 3 separate presses = 3 steps; 10 repeat events with a merge from a second replica after the 5th = 2 steps. Before `0020` exists a held press writes one `translate_objects` commit per event (there is no undo, so the maker sees no difference); with `0020` the commits of a run form one step, wired through the continuation of `Document::continue_step()` by whichever of the two slices merges second.
10. Given a nudge that would put any coordinate of any selected object beyond ±1e7 mm, or make the result invalid, then nothing moves, nothing is written, and the text "Too far from the document. Nothing was changed." appears in the canvas notice slot (`0020` UX notes) for 2 seconds, because the key may be pressed with the pointer outside the canvas (the rule of `0019` criterion 22).
11. Given the arrow key handled, then the page does not scroll and the canvas does not pan (`preventDefault`); given the arrow key not handled (nothing selected, another tool, a focused control), then the key keeps its meaning there (a value field's stepping, the rail's Up and Down, the colour picker).
12. Given an Escape during a held nudge, then the objects stay where the run has moved them; Escape does not undo a nudge (it is committed; `0020` undoes it).
13. Given the Node tool, then arrow keys do nothing to nodes in this slice (out of scope).

### Discoverability

14. Given the Select tool's tooltip on the tool rail, then it has a second, muted line "Arrows nudge 1 mm, Shift 10 mm. Ctrl+A selects all." under the existing text, and the native Edit menu, once it exists (`0020` criterion 8), has "Select All" with Ctrl+A. A `?` overlay is not part of this slice.

### Delivery

The slice is one branch `story/editing-quick-wins`, one PR, in three milestones (`adrs.md`): (1) a cache of the object read per document version, needed so that Ctrl+A on 5,000 objects is drawn within 100 ms (criterion 5), with an `#[ignore]` benchmark; (2) the nudge rule, the shared limit check and the key rows with session tests; (3) the frontend (the key time stamp, the outcomes, the readout, the tooltip line, the shortcut table rows). The slice does not run in parallel with `0043` or `0020` (same files).

## Out of scope

- Nudging nodes, handles or segments in the Node tool; a keyboard bend (`0031` notes propose it with this story).
- A preference for the distances, a grid or snapping, Alt for one screen pixel, a nudge in the display unit.
- Select all in the Node tool (all nodes of the path), Select all in all layers, Select same fill or stroke, Invert selection, Ctrl+Shift+A.
- Duplicate in place (Ctrl+D) and flip (H, V): also "Reserved, not built"; each is its own proposal.
- A keyboard help overlay (`?`).
- Undo and redo themselves: `0020-undo-redo`.

## Open questions (customer; each has a default)

1. **Do you want the nudge at all** (it is a scope growth, not a customer request)? *A (default, recommended):* yes, 1 mm and 10 mm. *B:* the Inkscape feel, 2 mm and 20 mm. *C:* not now; Ctrl+A only. The values are one constant each.
2. **Select all in other tools (criterion 3).** *A (default):* ignored. *B:* switches to the Select tool and selects all (Inkscape keeps the tool and selects). Recommendation A: no tool changes by surprise.

## UX notes

Written by the ux-engineer, 2026-10-10. No new control, no change to the Escape cascade. The table rows are in `docs/design-system.md`, "Keyboard shortcuts established so far" and "Keyboard concept".

### Feedback

| Key | What the maker sees |
|---|---|
| Ctrl+A | The selection box and handles appear (or grow) in the frame of the key; the subject line reads "5 objects"; when the selection was empty the Properties panel goes from Document to Style (`0043`, unless the maker pinned Document or History). No notice and no hint, also with an empty document (criterion 2). For a screen reader the visually hidden live region says "Selected 5 objects." |
| Arrow, Shift+Arrow | Objects and selection box move in the frame of each key event. A **move readout** next to the box says how far: the chip of the "Live transform readout" surface (`--toolbar-bg`, 8 px radius, 12 px tabular text, `pointer-events: none`, `aria-hidden`), text "Δ 3.0, 0.0 mm" in the move format (X right, Y down, one decimal, real minus sign), placed like the typed-move chip, 16 px right of and below the selection box centre, clamped into the viewport. It shows the **distance of the current step** (what Ctrl+Z would take back), appears with the first move of a step, and goes 800 ms after the last one. It is always in millimetres: the nudge is defined in document millimetres, not in the display unit, and "Δ 0.0394, 0.0 in" would only confuse. |
| End of a step | One visually hidden announcement, "Moved 11 mm right." or "Moved 3 mm right, 1 mm down." (`role="status"`), so a screen reader is not read every repeat event |
| Too far | The canvas notice slot (`0020`, UX notes) shows "Too far from the document. Nothing was changed." for 2 s; nothing moves |

Why a readout: at 10 % zoom a 1 mm nudge moves the artwork by about a third of a pixel, and a maker who presses an arrow key must be able to tell that it did something. The number costs no control and no permanent space.

No acceleration for a held key: 1 mm per repeat event (about 30 mm per second), 10 mm with Shift. The distance a hold travels is predictable and the run is one step.

### Key gate: three kinds of key, one gate

This story and `0020` introduce keys that are not the letters of `0010`. The design system's "Keyboard concept" now names three classes; all pass through the same gate function.

| Class | Keys | Modifiers | Key repeat | Gate (all of them) |
|---|---|---|---|---|
| Letters | tool and selection letters | none (Shift only for `*`, Shift+K) | ignored | No drag in flight, no chip open, no text field, select, button, switch or contenteditable focused, no IME composition, no unfinished Pen path, Space not held, no long operation |
| Chords | Ctrl or Cmd with a letter: Z, Shift+Z, Y, U, Shift+U, A | Ctrl or Cmd required; Alt not allowed | Z, Shift+Z, Y, U, Shift+U act on every repeat event (`0020` criterion 5); A once | The same list; a focused panel control that is not a text field (a value field in its spinbutton state, a toggle) does not block the chords, a typing field does |
| Arrows | Arrow, Shift+Arrow | Shift allowed; Ctrl, Cmd, Alt: ignored with no `preventDefault` | acts on every repeat event | The same list, and the Select tool with a selection |

An ignored key gives no feedback (no hint, no `preventDefault`), except where a criterion names a hint. A handled arrow key calls `preventDefault` so the page does not scroll; an unhandled one leaves the key to its control (value field stepping, rail Up and Down, picker).

### Discoverability

- The Select tool's rail tooltip gets a second, muted line: "Arrows nudge 1 mm, Shift 10 mm. Ctrl+A selects all." The tool is the one that owns both keys, and the tooltip is where the maker already looks for "S or Esc".
- The native Edit menu has "Select All" with Ctrl+A (`0020` UX notes).
- A `?` overlay remains its own story.

### Criteria changes requested (PO applies them)

- **4:** add: a screen reader hears "Selected 5 objects." through the visually hidden live region.
- **9:** add: while a step runs the move readout of the notes shows the step's distance; after the step ends a hidden live region says "Moved 11 mm right."
- **10:** the hint text appears in the canvas notice slot for 2 seconds (the key may be pressed with the pointer outside the canvas).
- **New criterion:** the Select tool's rail tooltip names the keys (second line above).

### Criteria changes requested and applied

All changes the ux-engineer requested (criteria 4, 9, 10 and the new tooltip criterion 14) were applied to the criteria above by the product owner on 2026-10-10.

### Design questions: decided (defaults, 2026-10-10)

1. Readout in millimetres whatever the display unit: decided yes.
2. The readout stays 800 ms after the last move: decided yes; shorter makes a single tap unreadable.

## Links

Requirements: R-EDIT-026
Builds on: `specs/0004-canvas-navigation-and-selection/`, `specs/0010-edit-interaction-polish/` (keyboard concept, gate), `specs/0019-multi-object-transform/`
Interacts with: `specs/0020-undo-redo/` (step coalescing), `specs/0023-groups/` (context), `specs/0039` layers (hidden and locked)
ADRs: `adrs.md` (architect): no new ADR; the held nudge uses the continuation of ADR 0014 §2 (Proposed)
PR: -
