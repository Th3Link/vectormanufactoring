# Undo and redo: one step per interaction, per-user stacks, a document history with step ids

Status: Ready (2026-10-10). The customer accepted ADR 0014 (`docs/adr/0014-history-undo-and-branches.md`) and every default of the open questions below on 2026-10-10. `adrs.md` and the UX notes exist; the criteria are complete and testable. Milestone 3 needs `0043` (merged with #84).
Priority: Must
Origin: Customer (specification of 2026-10-10, final: per-user undo and redo in the session, a global history of the document, Ctrl+Z and Ctrl+Shift+Z, step hashes or a shorter form, the kind of operation visible, deletions visible, wipe of the document history). Requirements R-EDIT-008 (the confirmed MVP slice 8) and R-HIST-001, R-HIST-004. Everything marked **Proposal** is the product owner's idea and is not accepted until the customer says so. Object history, object undo and branches are the follow-up specs `0041-object-history` and `0042-history-branches`; the right-panel tabs this spec needs are `0043-properties-tabs`.

## User value

As a maker, working alone or in a document shared with others, I want Ctrl+Z and Ctrl+Shift+Z to take back and bring back my own steps, one interaction per step, and I want a history list of every step anybody made in the document (who, when, what kind of operation, which objects, what was deleted), so that I can try things without fear and always see what happened in the file.

**What the reference tools do, and what we do better** (from their manuals and behaviour as far as we know; check against the version the customer uses):

- *Inkscape:* one global linear stack for the document; Edit > Undo History (Shift+Ctrl+H) lists the actions and a click jumps to one. No author, no time, no ids, nothing about collaboration. Here: the same list idea (and the same shortcut for it, Proposal below), but with author, time, operation and a step id.
- *LightBurn:* linear Ctrl+Z; to our knowledge no history list. The Boolean tools tell the maker to "undo and run again".
- *Affinity Designer and Photoshop:* a History panel; Affinity can save the history with the document and has a Clear button. This spec copies both ideas (history saved in the file, a wipe), and keeps the history list inside the Properties panel instead of a floating window.
- *Figma, Google Docs, Zed:* undo is per person in a shared document. ADR 0009 §1 already decided that for us (accepted by the customer); this spec makes it testable.
- *Git:* short ids for steps. We use an abbreviation of a stable id, lengthened when two would collide, as git does.

What is better than Inkscape and LightBurn: undo that cannot destroy a collaborator's work (criteria 15 to 20), a history that says who did what and keeps deleted objects visible, and a step id the same on every machine.

## Words used below

- **Step:** the document change made by one interaction, written as exactly one commit (ADR 0002 §9). One drag, one typed entry, one panel edit, one command press is one step. It is the unit of undo and one row of the history.
- **Session:** the time between New or Open and the next New, Open or quit. Each session has its own peer id (`Document::new(peer_id)`; `specs/0001-project-file-foundation/adrs.md`). A file reopened later is a new session.
- **Undo stack, redo stack:** the maker's own steps of this session, in order. Kept in memory per session, not saved.
- **Global history:** every step in the document's operation log, of every peer and every session. Saved in the file (criterion 37).
- **Operation name:** the user-facing name of a step ("Move", "Union", "Resize document"), from the table below.
- **Step id:** a short id, 7 characters or more (criterion 33).
- **Touched objects:** the objects a step created, changed or deleted.
- **Peer:** another session editing the same document at the same time (collaboration is not built yet; its rules here are tested with two document replicas that merge).

## Acceptance criteria

### Keys and commands

1. Given at least one step on my undo stack and focus on the canvas, the tool rail or the Properties panel (not in a text field), when I press Ctrl+Z (Cmd+Z on macOS), then the latest step on my undo stack is taken back: each object it changed has the values it had before the step (conflicts: criteria 15 to 19), the step moves to my redo stack, the selection is set by criterion 23, and a one-line notice says "Undid: <operation name> (<objects>)." The notice is shown in the canvas notice slot (design system) when a key was pressed, and 4 px under the button when a panel button was pressed (criterion 9). It follows `0016` criterion 29: it takes no focus, has `role="status"`, lasts 3 seconds (5 seconds above 70 characters) and is replaced by the next notice. `<objects>` is the touched-objects summary of the row ("1 rectangle", "3 paths", "5 objects") and is left out when the step touched no object.
2. Given a step on my redo stack, when I press Ctrl+Shift+Z (Cmd+Shift+Z), then that step is applied again, moves back to my undo stack, and the notice says "Redid: <operation name> (<objects>)."
3. **Proposal (Question 2):** on Linux and Windows Ctrl+Y does the same as Ctrl+Shift+Z. Not on macOS (Cmd+Y belongs to browser history). The tooltips and menu name only Ctrl+Shift+Z.
4. Given an empty undo stack (or redo stack), when I press the key, then nothing in the document changes and the notice says "Nothing to undo." (or "Nothing to redo.").
5. Given the key is held, then each auto-repeat event takes back one more step (undo and redo are not gated against repeat, unlike the letter shortcuts of `0010` criterion 55). Test: 5 steps, 5 repeat events, all five are taken back, a sixth gives "Nothing to undo."
6. **Gate.** Given any of these states, the key is ignored: no change, no `preventDefault`, no notice. (a) A drag is in flight in any tool, the panel's value fields included (marquee, lasso, move, resize, rotate, skew, parameter handle, node or handle drag, pen handle, create-drag). (b) A typed-entry chip is open. (c) Focus is in a text field, select or contenteditable: the field's own text undo applies and the document is not touched. (d) A long operation is running (`0016` §9, `aria-busy`). (e) The Pen has an unfinished path: the key is ignored and a hint line says "Finish the path (Enter) or cancel it (Esc) first." for 2 seconds (Question 3).
7. Given a QWERTZ keyboard (German), then Ctrl+Z is the key that types "z" and Ctrl+Y the key that types "y". Keys are matched by the typed character, not by the physical position (`0010` keyboard concept: letters are layout-aware). Test: key events with `key: "z"`, `"Z"`, `"y"` on both layouts.
8. **Proposal (Question 5):** the native menu bar gets an Edit menu with "Undo <operation name>" and "Redo <operation name>" (plain "Undo" and "Redo" when the stack is empty, greyed as native menus do), with the accelerators of criteria 1 and 2. Exactly one handler fires: one press undoes exactly one step, and a focused text field still gets its own undo (checked by hand on each OS, because a native accelerator can swallow the key before the web view).
9. Given the History tab (criterion 39), then its header has an Undo and a Redo icon button, present only while the stack has an entry (the panel removes controls that cannot apply, `0017`). Each button has a reserved 28 px slot at the right of the header row: a button that cannot apply leaves the tree and its slot stays empty, so the other one does not move. Tooltips: "Undo: <operation name> (Ctrl+Z)" and "Redo: <operation name> (Ctrl+Shift+Z)". Pressing one is the same as the key.

### What a step is

10. Given any gesture in the table "One interaction, one step" below, when it is performed once, then exactly one step is added to the global history and to my undo stack, with the operation name of the table. Given Ctrl+Z, then the document is as before the gesture: the `document.json` export (objects, ids, anchor ids, styles, size, display unit) is byte-equal to the export taken before the gesture. Given Ctrl+Shift+Z, then it is byte-equal to the export taken after. One test per row.
11. Given a step that touches N objects (a drag of 3 objects, Delete of 4, a Boolean over 5, a style edit on 3), when I undo it, then all N are restored by the one press. No other number of presses is needed and no object stays half-changed.
12. Given an interaction that writes nothing, then it adds no step: Escape during a drag, a drag that ends where it began, a typed value equal to the current one (`0017` "writes nothing"), a refused operation (`0016` criterion 15, "Nothing was changed."), "Already fits the content."
13. Given two identical operations in a row on the same object (two moves, two style edits) within one second of each other, then there are two steps and two presses are needed. (The undo engine must not merge steps: Loro's undo manager merges steps by a time interval, and consecutive commits with one message can merge into one Loro change, `docs/technical-debt.md`, "A boolean operation is one commit, but one commit is not one Loro change".)
14. Given a step with a large result (a Boolean union whose result needs about 1,300 Loro operations or more, 5,000 anchors), then it is still one step, one id, one row, one press.

### One interaction, one step

The names are the user-facing form of the commit labels on `main` on 2026-10-10 (`commit_with_label` in `curvyo-document-core`, `COMMIT_LABEL` constants in `curvyo-ui-core`). The implementer verifies the list against the code; criterion 41 makes a missing entry a test failure.

| Gesture | Commit label | Operation name |
|---|---|---|
| Pen: finish a path | `create_path` | Draw path |
| Pen: continue, connect, close (`0034`) | `extend_path`, `connect_paths`, `close_path` | Extend path, Connect paths, Close path |
| Rectangle, ellipse, polygon, star tool: create-drag | `create_rect`, `create_ellipse`, `create_polygon`, `create_star` | Draw rectangle, Draw ellipse, Draw polygon, Draw star |
| Select: move drag, typed move (M), centre handle | `translate_objects` | Move |
| Select: Ctrl-copy move, duplicate | `duplicate_objects` | Duplicate |
| Select: resize, typed size (S), parameter handles, point count, inner ratio | `resize_path`, `resize_rect`, `resize_ellipse`, `resize_star_frame`, `set_rect_bounds`, `set_ellipse_frame`, `set_star_frame`, `set_point_count`, `set_inner_ratio` | Resize, Change point count, Change inner ratio |
| Select: rotate drag, typed angle (R) | `rotate_object` | Rotate |
| Select: skew drag, typed skew (K) | the label the skew commit uses today (the implementer names it) | Skew |
| Select: corner radius | `set_corner_radius`, `set_corner_radii` | Change corner radius |
| Select: Object to path | `convert_to_paths` | Object to path |
| Select: Delete | `delete_objects` | Delete |
| Select with several objects (`0019`): box drag or typed entry | the same labels as for one object, one step for all | as above |
| Properties panel, Style: any edit, one drag of a value field, one pick | `edit_style` | Change style |
| Properties panel, Document: size, preset, orientation, Fit to content, unit | `resize_document`, `fit_document_to_content`, `set_display_unit` | Resize document, Fit document to content, Change display unit |
| Node tool: drag nodes, drag a handle, change kind, insert, delete, segment line or curve, bend | `move_anchors`, `set_handle`, `convert_anchor_kind`, `insert_anchor`, `delete_anchors`, `set_segment_line`, `set_segment_curve`, `bend_segment` | Move nodes, Move handle, Change node type, Add node, Delete nodes, Straighten segment, Curve segment, Bend segment |
| Node tool: join, split | `join_endpoints`, `split_at_anchor` | Join nodes, Split path |
| Boolean toolbox | `boolean_union`, `boolean_difference`, `boolean_intersection`, `boolean_exclusion`, `boolean_reverse_difference` | Union, Difference, Intersection, Exclusion, Reverse difference |
| Path toolbox | `combine_paths`, `break_apart` | Combine, Break apart |
| Later specs | `group`, `ungroup` (`0023`); the background (`0040`); fracture, flatten, split, offset (`0036`, `0037`, `0038`) | Group, Ungroup, Change background, Fracture, Flatten, Split at crossings, Offset |

New, New project, Open and Save are not steps (criteria 25, 26). Selection, tool, pan, zoom, entered group and the panel's tab are view state: never a step (ADR 0009 §2).

### Who can undo what: collaboration

Tested with two document replicas A and B that merge in the test; no network is needed. With nobody else connected everything below reduces to a plain linear stack (ADR 0009 §1).

15. Given a shared document, then my undo stack and redo stack hold only steps made by my session. A step from a peer, or from an earlier session of this file, never enters them, never reorders them, and never clears my redo stack. Test: A makes steps 1 and 3, B makes step 2 between; A's stack is [1, 3]; B's step arrives while A has steps on its redo stack, the redo stack is unchanged.
16. **Conflict rule, per field.** Given I undo step S, then for each field S changed, the field is set back to its value before S **only if its current value is still the value S wrote**. A field a peer has changed since S keeps the peer's value. Test: A sets stroke width 1 to 2 (S); B sets it to 5; A undoes S; the width is 5 on both replicas. A fill A changed in the same step and B did not touch goes back.
17. Given a step touched an object a peer has deleted since, when I undo it, then that object is skipped. Undo never brings back what a peer removed (ADR 0009 §1).
18. **Proposal (Question 4):** given I created an object (step S) and a peer has changed it since, when I undo S, then the object stays and the notice says "Not undone: someone else changed this object after you drew it." Without the proposal the object would be removed together with the peer's work.
19. Given an undo in which some fields or objects were skipped (criteria 16 to 18), then the notice says "Undid: <name>. N of M objects kept later changes by someone else." and, when everything was skipped, "Could not undo <name>: everything it changed was changed or deleted by someone else since." A press that skips the whole step still consumes that entry: it leaves the undo stack, goes nowhere, and the next press takes back the next entry. (One press, one entry, so the maker is never surprised by a step they did not see.) Such a press writes nothing to the log (ADR 0014 consequences), so the History list shows the step as Applied: its effect was overwritten by others.
20. Given my undo of a step, then it is itself recorded as a change in the operation log and replicates like any edit: after the merge, B shows what A shows. The log only grows (ADR 0009 §1). Test: A undoes, merge into B, `document.json` of A and B equal.
21. Given my undo, then a peer's undo stack and redo stack are unchanged.

### After an undo

22. Given I undo one or more steps and then make a new step, then my redo stack is emptied: Ctrl+Shift+Z says "Nothing to redo." The steps that were taken back stay in the global history, marked "Undone". They are not lost: `0042-history-branches` shows them as a branch and lets the maker return to them.
23. **Selection.** Given an undo, then the selection becomes what it was before the step (ids that no longer exist are dropped); given a redo, what it was after the step. Test: select two squares, Union, Ctrl+Z: the two squares are selected; Ctrl+Shift+Z: the result is selected. Undo of Delete brings each object back with the **same id** (and the same anchor ids and z-order; ADR 0014 §11, injected tree Move, Question 17) and selects the restored objects. Test: draw, Delete, Ctrl+Z, Save, Open: the `document.json` object ids equal those before the Delete. The tool does not change. In the Node tool the selected nodes are restored by anchor id, ids that are gone are dropped. The selection is kept with the stack entry in memory (view state, ADR 0009 §2), not in the document.
24. Given an undo or redo, then pan and zoom do not change. When none of the restored selection touches the visible canvas, the tail ", off screen." replaces the full stop of the notice, after the closing parenthesis: "Undid: Delete (2 rectangles), off screen." (no second pair of parentheses). (Question 8: panning to the objects instead.)

### Boundaries and limits

25. Given New or Open, then both stacks are emptied and no step is created. After Open of a file that has history, Ctrl+Z gives "Nothing to undo." (criterion 31).
26. Given Save or Save As, then the stacks are unchanged: Ctrl+Z after Save works and the saved file stays as saved.
27. Given quit or window close, the stacks are dropped; the global history stays in the file.
28. Each stack holds at most 500 steps (a constant). The 501st step drops the oldest entry from the stack; it stays in the global history. Test: 501 steps, 500 undos work, the 501st gives "Nothing to undo."
29. **Memory (starting value, the architect measures):** the undo data of 500 steps of the largest kind (a Boolean union of two paths of 2,000 anchors each) stays under 50 MB. If it does not, the architect lowers the step count of criterion 28, not this budget.
30. **Time (ADR 0014 §13; release build, desktop; the tests assert four times each figure; a wasm figure is measured in milestone 1 and recorded as a factor, not a separate budget).**
    - (a) An undo or redo of a **small step**, at most 2,000 operations (read from the step's change lengths before the call; this covers every single-object edit and moves of a few hundred simple objects), completes and is drawn within 100 ms and shows **no** busy indicator.
    - (b) A **larger step** (more than 2,000 operations) enters the long-operation state of `0016` §9 **before** the call (cursor `wait`, `aria-busy`, other keys ignored per criterion 6d, "Undo: working..." after 150 ms). It completes and is drawn within 250 ms for a step touching 100 paths of 100 anchors each, and within 1 second for 5,000 objects.
    - (c) **First undo after Open.** The editor warms the history in an idle callback after the first paint. An undo pressed before that has finished uses the state of (b), whatever the size of the step.
31. Given a file opened after a restart, then the global history lists its steps (criterion 37) and both stacks are empty.

### The global history: what is recorded

32. Every step has: a step id, an author, a time, an operation name, a kind, its touched objects (ids and kinds), and a status. **Kind** is derived from what the step did, not from the name: only created objects is *Create*, only deleted is *Delete*, created and deleted or moved to another parent is *Replace*, only changed is *Change*, none is *Document*. The editor API returns these for the panel; the panel and the tests read nothing else.
33. **Step id.** The id is 7 characters of lowercase letters and digits. It is the same on every replica of the document, stable across Save and Open, and unique within the document: when two steps would share the first 7 characters, both are shown with as many more characters as it takes to tell them apart (as git abbreviates). Test: save, reopen, ids equal; merge two replicas, ids equal; a set of 20,000 generated steps has no duplicate shown id. Derivation (ADR 0014 §3): the id of a step is the id (peer, counter) of its first operation; the shown id is its FNV-1a 64 hash in base36, lowercase, 7 characters, lengthened on a collision within the document. A test pins one known (peer, counter) to one known id.
34. **Time.** Each step carries the local wall-clock time of its commit. The editor passes the time in (wall-clock seconds); the core crate reads no clock (`CLAUDE.md` §6). A step from a file written by an earlier build has no time (stored as 0) and the row says "Unknown time".
35. **Author.** Each step carries its author as the peer id of the session (a random number per session, ADR 0014 §4). The row shows "You" for steps of this session and "Earlier" for every other peer id; the tooltip and the accessible name of the row say "Earlier session" (ADR 0014 Q5: the alternatives are one id per installation or the OS user name). Names come with collaboration identity (ADR 0004, "Collaboration identity"); until then no personal data goes into the file.
36. Given a file from an earlier build (for example `compound_v8.curvyo`), then the history lists its steps with operation names read from the persisted commit labels, author "Earlier session", time "Unknown time". A label the table does not know shows the operation name "Edit" and the raw label in the tooltip. Two adjacent commits with the same label from such a file show as one row, and the first row also holds the document's root setup (ADR 0014 consequences; the release notes say so).
37. **Saved in the file.** The global history is the operation log inside `document.loro` that the file already holds. No new member of the container, no `format_version` bump (ADR 0014 Q2 A, the default; test: a file saved by this build has the `format_version` of the build before it). After Save and Open, the list is the same: ids, order, names, authors, times, kinds. The undo and redo stacks are not saved.
38. **Deletions are visible.** Given Delete of two rectangles, then the history has a row "Delete" with the kind *Delete* and the summary "2 rectangles" after the objects are gone. A Boolean shows its operands as deleted and its result as created in its one row. Test: after Delete, Save, Open, the row is still there with the same summary.

### The History tab (needs `0043-properties-tabs`)

39. Given the Properties panel, then a third tab "History" shows the global history as one scrollable list, newest at the top (Question 7).
    - The first row is the pinned row "Now": it is not a step, is not counted among the steps (not among the 10,000 of criterion 43), is never scrolled away, shows the id of the newest applied step, and is the selected row while nothing is previewed (`0042`).
    - Every step is a row of a fixed height of 44 px with a lane gutter at its left (one lane in this spec; `0042` adds branch lanes). A row shows: a kind glyph (not by colour alone), the operation name, the step id (monospace, never truncated, selectable text), and a second line with the status word (only when not Applied), the touched-objects summary, the author and the time, separated by " · ".
    - Time in the row is the clock time ("12:03"), with the date before it for steps older than today ("9 Oct 12:03"). It is never relative ("2 min ago"), because rows must not change under the pointer. The tooltip has the full local date and time with seconds.
    - Status. Applied has no word. A step I took back with Ctrl+Z shows "Undone". A step taken back by anyone else, or by an object undo (`0041`), shows "Undone by <author>" ("Undone by You" for my own object undo). Undone is shown by an open ring on the lane, a dotted lane line and the word, not by dimming alone.
    - No new row is added for an undo or redo: the log holds the undo commit (ADR 0009 §1), the list shows it as a status of the original step.
40. Given the pointer rests on a row, or a row has keyboard focus, then the touched objects that still exist show the hover box of `0014` on the canvas, and lose it in the frame the pointer leaves or focus moves. Deleted objects show nothing.
41. **Guard.** A test lists every commit label in the source (`commit_with_label` calls and `COMMIT_LABEL` constants) and fails when one has no entry in the operation-name table. A new command therefore cannot ship without a name.
42. Keyboard: the list is one Tab stop with roving focus (`ToggleGroup` rule of the design system): Up and Down move, Home and End jump, Page Up and Page Down move by a page; Enter or Space presses the row (preview, `0042`); Right moves into the row's action button (`0042`) and Left back to the row; Escape ends a preview, a second Escape returns focus to the canvas. Each row has an accessible name in the form "Move, 3 objects, You, 12:03, id a1b2c3d, undone". A step by another peer does not announce itself (no live region for the list); the notice of criterion 1 is the only announcement.
43. **Scale.** Given a document with 10,000 steps, then the first 50 rows show within 200 ms of opening the tab and the list loads further rows while scrolling. Rows outside the viewport are not in the DOM. The list is the one inner scroll area of the panel: the tab strip, the header (Undo, Redo, scope) and the footer (the saved-in-file line and the wipe control) stay in view while the list scrolls; every other tab scrolls as a whole. Test: 10,000 steps built through the core API.
44. Given a new step of this session, then its row appears at the top in the frame of the commit. A list that has been scrolled away from the top keeps its scroll position.
45. Given a new document, then the list says "No steps yet." and, under it, in a muted line, "Steps appear here as you edit." (the Now row stays pinned). Given the tab, then a line below the list says "The history is saved in the project file." (it tells the maker what a shared file contains; Question 9).
46. Given Ctrl+Z or Ctrl+Shift+Z, then Shift+Ctrl+H (Proposal, Inkscape's key for its Undo History) shows the panel's History tab, expands the panel when collapsed, and moves focus to the list. Ignored during a drag.

### Wipe the document history

47. Given the History tab in the Document scope and at least one step, then a control "Wipe history" is the last element (the footer). Given no step, then the control is not in the tree. Pressing it replaces the control in place by the line "Remove all N steps from this file? The drawing stays as it is. This cannot be undone." with the buttons "Cancel" (left) and "Wipe" (right), in the panel (no dialog, no popup, `0017`). Keyboard focus goes to Cancel. "Wipe" ignores presses for the first 400 ms, so a double-click on the old control cannot confirm. Escape, Cancel, a change of tab, a change of selection and a new step cancel the confirmation without writing anything. Nothing happens until "Wipe".
48. Given "Wipe", then: the drawing is exactly as before (`document.json` byte-equal, object ids unchanged); both stacks are emptied; the history shows one row "History wiped" with author and time (the wipe writes the one root key `history_wiped`, ADR 0014 §7; it is the one operation that makes the row exist); steps made afterwards follow it. The object ids and the peer id are unchanged, so a step made after the wipe has the same author as before it. Test: build 200 steps, wipe, export the snapshot, open it in a new document: the history has one row, the objects are equal.
49. Given a wipe and Save, then the saved `document.loro` holds no operation older than the wipe (the architect's mechanism, for instance a shallow snapshot; the observable test is the number of history rows after Open and the file size on a document with 500 steps, which is smaller). The wipe itself is not a step on any stack and cannot be undone.
    - **Wipe epoch (ADR 0014 §9).** The wipe writes the root key `history_wiped` = the epoch (the encoded start of the kept history), the same on every replica that has the wipe. Given an exchange of updates (the `import_updates` seam), then the epochs are compared first: a replica of another epoch (for instance a copy that still holds pre-wipe steps) is refused with a clear error, and nothing of its updates is merged. An import that leaves updates `pending` is an error too. Test: wipe replica A, then offer B's pre-wipe updates to A: refused, A unchanged; A and B with equal epochs merge as before.
50. **Shared documents.** Given a document that has been shared (it has a keyring, ADR 0004 §1), then "Wipe history" is not offered and the line "Wiping is not available for shared documents yet." takes its place. (Hard erase of a replicated history needs every peer and the admin rules of ADR 0010; Question 6.)
51. Given a drag, an open chip or the Pen with an unfinished path, then "Wipe history" is ignored, like criterion 6.

### Text that says "no undo"

52. Given this slice is done, then no text the maker can see says "No undo yet" or "no undo" (notice, tooltip, hint, note line). Delete these places, and update the tests that pin them:

| Where | What to do |
|---|---|
| `frontend/src/lib/penText.ts`, `pathText.ts`, `booleanText.ts` | Remove the sentence "No undo yet." from every notice and note line; the success notices end after the count |
| `frontend/tests/penText.test.ts`, `pathText.test.ts`, `booleanText.test.ts`, `booleanTextSpec.test.ts`, `pathToolsSpecText.test.ts` | Update the pinned strings |
| `specs/0016`, `0034`, `0035`, `0036`, `0037`, `0038`, `0023`, `0019`, `0031` | Their criteria and text tables say "No undo yet." until `0020`: the implementer of this spec removes the phrase in the same PR and states in each spec's Status line that it was amended by `0020`. `0019` criterion 22 loses "There is no undo yet, so": a release at factor 0 is then taken back with Ctrl+Z |
| `docs/design-system.md` | The ux-engineer removes the notice wording in the Boolean and Path toolbox rows, the Escape-cascade row ("no undo for it until `undo-redo`"), the rows "Object to path", "Boolean operations", "Combine, Break apart", "Close path" (no shortcut until undo exists: decide with this spec, Question 10), and adds the undo, redo, nudge and History rows |
| `docs/technical-debt.md` | The entry "A boolean operation is one commit, but one commit is not one Loro change" gets its Resolution (criteria 13 and 14) |
| Done specs `0007`, `0008`, `0009`, `0013`, `0017` | Stay as the record of what was built; each gets a one-line "Superseded by `0020`" note where it says there is no undo |

53. Given a later spec adds a command, then it ships with an entry in the operation-name table, a test for "one interaction, one step", and the line "Undo: Ctrl+Z" in its acceptance criteria; the spec template of `specs/README.md` does not change.

### What the engine must guarantee (ADR 0014, accepted; testable)

These criteria pin the decisions of ADR 0014 that the maker or a tester can observe. They hold on the options the customer accepted (Questions 11, 12 and 17 below, option A).

54. **A step is one commit with a header.** Given any gesture of the table, then its one commit carries the header `<label>;s=<seq>` in the commit message (ADR 0014 §2), and a step is all commits of one peer with the same header. A **continuation** commit repeats the header of the peer's last step and joins it; it is allowed only while the document version is still that step's last version, and refused (it becomes a new step) when anything was committed or merged since. Test: 11 continuation commits are one row and one stack entry; the same 11 with a merge between the 5th and the 6th are two steps. A commit message without `;` is a legacy label (criterion 36).
55. **One stack machine.** Given a sequence of do, undo, redo and continuation events, then the undo stack, the redo stack, the status of every row ("Applied", "Undone", "On a branch" for `0042`) and the branches come from one pure transition function (ADR 0014 §6). The session feeds it its own actions; the History list feeds it the log. Test: a generated sequence of 1,000 events; after each event the live stacks equal the stacks and statuses derived from the log of the same sequence.
56. **The undo is a step in the log, derived, not stored.** Given an undo or a redo, then it is one commit with the header `undo;s=<n>;u=<step id>` (or `redo;…;d=<step id>`), replicated like any edit (criterion 20). Given Save and Open, then every row's status ("Undone", "On a branch") is the same as before; nothing is stored for statuses or branches beyond the log.
57. **The first step does not undo the document's setup.** Given a new document and one step ("Draw rectangle"), when I press Ctrl+Z, then the `document.json` export equals the export of the new document before the step, with `format_version`, the document size and the display unit intact. `Document::new` commits its root setup as its own commit with the reserved label `new_document`; that commit is listed nowhere and never on a stack (ADR 0014 context). Test: the export after Ctrl+Z of the first step equals the export of the empty document.
58. **One commit per gesture.** Given the resize, rotate or skew of several objects (`0019`, which today writes one commit per object), then it is one commit and one step for all (milestone 1 fixes `curvyo-ui-core`'s `transform_commit.rs`). Generally, every row of the table "One interaction, one step" is checked by a test that counts the commits of one gesture: exactly 1 (a held nudge of `0044` is the one allowed exception, many commits and one step, criterion 54).
59. **No format version bump; optional keys.** Given a file saved by this build, then its `format_version` is the one of the build before it (ADR 0014 Q2 default A). The only additions to the file are optional keys that a build which does not know them ignores: `history_wiped` (this spec, criterion 48), and `history_floor` and `clone_of` (`0041`). A document without them is valid. Test: a document written with these keys loads in a reader that ignores unknown keys, and the drawing is equal.
60. **A step id names a step on every replica.** Given two replicas that merge, then a step has the same id, author, time, kind and touched objects on both. Given a step by a peer that is in the log but whose objects are gone, then its row still shows the kind and the object summary (criterion 38).

## Milestones (one branch, one PR)

0. **Spike** `spike/loro-history-primitives` (ADR 0014 "Verification", S1 to S7), never merged. Done: S1 (restore a deleted node with its old id) fails through the handler API and works through an injected tree Move (ADR 0014 §11); the customer chose that way (Question 17 A, 2026-10-10).
1. **Engine:** step header and step time, the `new_document` commit, the commit-path audit with one test per row of the table (criteria 10, 41, 57, 58), `batch` where a gesture has several commits, the restore engine, the stack machine, keys, gate, conflict rules, selection, limits, the removal of the "no undo" text (criteria 1 to 31, 41, 52, 54 to 58). The slice that is the old MVP slice 8; Ctrl+Z works end to end here. The architect reviews the branch diff here (document model, public API).
2. **Step data:** ids, time, author, kinds, old files, persistence (32 to 38, 59, 60).
3. **History tab:** the list, hover link, keyboard, scale (39 to 46). Needs `0043`.
4. **Wipe** (47 to 51). Builds on ADR 0014 Q3 A (unshared documents only).

The customer accepted ADR 0014 and these defaults on 2026-10-10; no milestone waits for an answer.

## Notes for the architect (answered by ADR 0014 and `adrs.md`)

- ADR 0002 §9 and ADR 0009 §1 stay the accepted base: the operation log is the history, undo is a forward commit, peer-scoped. ADR 0014 §1 supersedes the sentence of ADR 0004 §2 that names Loro's undo manager as the mechanism: the engine is our own restore over versions, because the manager has no per-object veto (criteria 17 to 19).
- Step boundaries are a header in the commit message (not Loro's change boundaries, which split and merge: criteria 13 and 14). The kind and the touched objects are derived from the step's operations (criterion 32).
- Undo reads the step's before and after values with `diff` and the current values live, so it does not check out (ADR 0014 §1); checkouts are only for whole past states, behind a guard (§12). The budgets of criterion 30 are ADR 0014 §13.
- Undo of Delete needs a pinned Loro version and a canary test (ADR 0014 §11, Question 17). Risks the architect measures in milestone 1: the budgets of criterion 30 with the restore writes included, the canary, the nested containers of deleted objects (spike S6), old files with merged rows and no times (criterion 36).
- `docs/technical-debt.md` "Document files grow with edit history": the wipe is the first user-visible answer; the compaction story stays separate.

## Out of scope

- Object history, object undo and redo (Ctrl+U), per-object wipe, clone with history: `0041-object-history`.
- Branches, the live and preview switch, restoring a version, clone of a state: `0042-history-branches`. Until then the redo stack is simply emptied (criterion 22).
- Undoing a collaborator's step from the history (ADR 0009 option C): `0041` does it for one object's last step; nothing here does.
- Unsaved-changes tracking (a dirty marker, "save before closing?"): there is none today (`0001`); when it comes, undo back to the saved state makes the document clean again.
- Undo of an unfinished Pen path's single points (Question 3, default: not built).
- Autosave, crash recovery, named versions, snapshots, export or print of the history, a time slider.
- Undo of view state, selection history, tool changes.
- Names and colours of collaborators in the list (collaboration identity, ADR 0004).
- A step count the maker can configure (YAGNI; a constant).

## Open questions (decided: the customer accepted every default, 2026-10-10)

Each question below is decided as its option A (or its stated default). The text stays as it was put.

1. **Persisted history (R-HIST-001).** The file already carries the full log, so "the global history is saved" costs nothing and needs no format change. The consequence: whoever gets the `.curvyo` sees every step, including deleted objects. *A (default):* saved, with the line in the History tab (criterion 45) and the wipe (criteria 47 to 51). *B:* strip the history on every Save (the history lives only for the open session). Recommendation A: it is what the customer asked for. (Same as ADR 0014 Q1, Question 11 below.)
2. **Ctrl+Y (criterion 3).** *A (default):* yes on Linux and Windows, not macOS. *B:* only Ctrl+Shift+Z. Recommendation A.
3. **Ctrl+Z while the Pen has an unfinished path (criterion 6e).** *A (default):* ignored, with a hint. *B:* removes the last placed point (Inkscape's behaviour), and is the first point of the next press when none is left. Recommendation A for this slice; B is a small follow-up.
4. **An object a peer edited after my step (criterion 18).** *A (default):* the object stays (the peer's work is protected). *B:* undo removes it anyway. Recommendation A. (Author names are ADR 0014 Q5, Question 15 below.)
5. **Edit menu (criterion 8).** *A (default):* add it. *B:* keys only. Recommendation A: a native menu is free accessibility and shows the operation name.
6. **Wipe in a shared document (criterion 50).** *A (default):* not offered until the sharing slice. *B:* offered to admins only (ADR 0010), every peer must re-sync. Recommendation A. (Same as ADR 0014 Q3, Question 13 below.)
7. **List order (criterion 39).** *A (default):* newest at the top (nothing to scroll to see the last step). *B:* oldest at the top, as Inkscape's Undo History. Recommendation A, the UX review may change it.
8. **After an undo that restores something off screen (criterion 24).** *A (default):* the view stays, the notice says "(off screen)". *B:* pan to the objects. Recommendation A: the view is the maker's.
9. **Sending a file with history.** *A (default):* the History tab says the history is in the file. *B:* the Save As dialog offers "without history". Recommendation A for now; B belongs to export and sharing.
10. **Shortcuts for destructive commands** (Boolean, Combine, Break apart, Object to path, Close path had none "until undo exists"). *A (default):* decide in the Boolean-shortcut follow-up, not here. *B:* give them Inkscape's keys now. Recommendation A.

### The seven questions of ADR 0014 (decided 2026-10-10: option A each)

ADR 0014 is accepted; the customer took option A of every question below. They are the same seven questions in `0041` and `0042`.

11. **History in the file (ADR 0014 Q1; Question 1 above).** *A (default):* keep it (it is there today), the History tab says so, a wipe is available. *B:* strip the history on every Save.
12. **File-format footprint (ADR 0014 Q2).** *A (default):* no `format_version` bump; step headers in commit messages and three optional keys (`history_wiped`, `history_floor`, `clone_of`) that older builds ignore. *B:* bump the version at `0041`, so older builds refuse files that carry lineage keys.
13. **Wipe in a shared document (ADR 0014 Q3; Question 6 above).** *A (default):* not offered until sharing exists. *B:* admins only, with a new key epoch and a fresh sealed snapshot (ADR 0010 §8); copies other people already hold keep the old history. *C:* no wipe at all.
14. **Object wipe (ADR 0014 Q4; `0041` Question 3).** *A (default):* it hides and cuts, the data stays in the file, the confirmation says so. *B:* no object wipe.
15. **Authors (ADR 0014 Q5).** *A (default):* a random id per session, shown as "You" and "Earlier session". *B:* one random id per installation in every step, so "You" survives reopening, but every file you share carries the same identifier. *C:* the OS user name in the file.
16. **Taking back someone else's step (ADR 0014 Q6; `0041` Question 1, `0042` criterion 15).** With Ctrl+U or go to version, never with Ctrl+Z (ADR 0009 option C). *A (default):* allowed, as a named step that Ctrl+Z takes back. *B:* own steps only. (No effect in this spec: Ctrl+Z is own steps only.)
17. **Undo of Delete (ADR 0014 Q7; criterion 23).** *A (default, recommended):* the object comes back with the **same id** through an injected Loro tree Move (ADR 0014 §11); no file-format change; Loro stays pinned and a canary test guards every upgrade; soft delete stays the fallback and comes back to the customer only if a Loro upgrade we need breaks the canary. *B:* soft delete now: deleted objects move into a hidden trash inside the file; public Loro API only, but a `format_version` bump (older builds refuse new files) and deleted objects stay in the file until a wipe. *C:* the object comes back under a **new id**: no risk, but selections, clone lineage and the object's timeline break at every undone delete. Not recommended.

## UX notes

Written by the ux-engineer, 2026-10-10. Component numbers are in `docs/design-system.md`, "Properties panel: History tab", "List row", "Inline confirm", "Canvas notice slot". `0041` and `0042` add to the same tab; their notes say only what they add.

### The tab at a glance

```
[File][Droplet][History]              24 steps     strip row, subject line (0043)
[Document|Object]                [Undo][Redo]      scope group (0041) and two reserved 28 px slots
Mode  [Preview|Live]                               Object scope only (0042)
A click shows the version. Nothing changes.        caption, or "Restore previous state" (0042)
----------------------------------------------
 Now                              a1b2c3d          pinned row, not a step
 ● [+] Draw rectangle             a1b2c3d          44 px rows, newest first
       1 rectangle · You · 12:03
 ...                                               the one scroll area, virtualised
----------------------------------------------
The history is saved in the project file.          footer, always in view
[ Wipe history ]
```

The tab is the one place in the panel with an inner scroll area, and the reason is the list: with 10,000 rows the header (Undo, Redo, scope) and the Wipe button would be thousands of pixels apart if the panel scrolled as a whole. The strip, the header rows and the footer stay in view; only the list scrolls (thin scrollbar, `overscroll-behavior: contain`). Every other tab scrolls as a whole.

### Row anatomy (244 px wide, 44 px high, fixed so the list can be virtualised)

| Part | Rule |
|---|---|
| Lane gutter | 16 px with one lane (the main line), +12 px per branch lane, at most 3 branches (`0042`). Width is set by the most lanes any row of the list needs, so rows do not shift while scrolling |
| Kind glyph | 16 px Lucide, 1.5 px stroke, `--toolbar-icon`, the kind of criterion 32: Create `Plus`, Delete `Minus`, Replace `Replace`, Change `Pencil`, Document `File`; and for `0041`: object undo `Undo2`, object redo `Redo2`; the wipe `Eraser`. Shape, never colour. The operation name carries the detail; 40 operations do not get 40 glyphs (calm) |
| Line 1 (20 px) | Operation name, 14 px `--toolbar-icon`, ellipsis; at the right the step id, 12 px monospace `--panel-muted-fg`, 7 characters or more, never truncated. The id text is selectable (`user-select: text`, double-click selects it) so it can be copied |
| Line 2 (16 px) | 12 px `--panel-muted-fg`, ellipsis from the left part: **status word** (semibold, only when not Applied: "Undone", "Undone by Anna"), summary ("3 paths"), author, time, separated by " · ". Author is "You" or "Earlier" in the row (the full "Earlier session" is in the tooltip and the accessible name). Time is the clock time ("12:03"); older than today "9 Oct 12:03". No relative time ("2 min ago"): rows must not change under the pointer |
| Row action | A 24 × 24 icon button at the right end of line 2, shown on hover, on focus within and on the selected row; reserved width 28 px so nothing shifts. Owned by `0042` (Clone, or the objects disclosure) |
| Ground | Hover: `--editor-accent-hover`. Selected (the row whose version is shown, `aria-current`): `--value-fill` ground and a 3 px `--accent` bar at the left edge; hover does not change a selected row. Focus: the panel ring, 2 px, inside the row |
| Status by shape and word | Applied: filled 8 px disc on the lane. Undone (redoable): 8 px ring on the lane, dotted line, name and glyph in `--panel-muted-fg` (5.0:1), the word "Undone". Branch rows and the fork connector are drawn by `0042`. Never opacity alone |
| Pinned row "Now" | 32 px, first row, not scrolled away, not a step: filled disc, the word "Now", the id of the newest applied step at the right. It is the target of "back to the present" (`0042` criterion 18) and the anchor of the lane. Selected when nothing is previewed |
| Tooltip | 400 ms, `side="left"`, text: "Move, 3 paths. You, 9 Oct 2026 12:03:41. Id a1b2c3d." plus the raw label for an unknown old label |
| Hover link | Pointer or keyboard focus on a row shows the hover box (`0014`) on the existing touched objects, and removes it in the frame the pointer or focus leaves (criterion 40) |

### Header

- **Undo, Redo:** two 28 × 28 icon buttons (Lucide `Undo2`, `Redo2`, 16 px) in two **reserved slots** at the right of the header row, as the value field reserves its reset slot: a button that cannot apply leaves the tree (criterion 9) and its slot stays empty, so the other one does not move. Names and tooltips as criterion 9. A mouse press returns focus to the canvas. The notice of the press sits 4 px below the button, right-aligned, 244 px wide at most (the panel-button rule of the "Action notice" row).
- The scope group and Preview/Live row belong to `0041` and `0042`.

### Notices

One element, the **canvas notice slot**, for everything a key produces (Ctrl+Z, Ctrl+Shift+Z, Ctrl+U, Ctrl+Shift+U, the Pen hint): centred in the free part of the viewport right of the rail, 12 px above its bottom edge, `--toolbar-bg`, 12 px text, 8 px padding and radius, `--panel-elevation-shadow`, no controls, no focus, `role="status"`, 3 s (5 s above 70 characters), replaced by the next notice. The reason it is not the status bar (display only) nor the pointer (a key press from the panel has no pointer on the canvas).

| Event | Text |
|---|---|
| Undo | "Undid: Move (3 paths)." |
| Redo | "Redid: Move (3 paths)." |
| Empty stack | "Nothing to undo." / "Nothing to redo." |
| Partly skipped | "Undid: Move. 1 of 3 objects kept later changes by someone else." |
| Skipped completely | "Could not undo Move: everything it changed was changed or deleted by someone else since." |
| Restored off screen | "Undid: Delete (2 rectangles), off screen." (the tail replaces the full stop; no second pair of parentheses) |
| Pen unfinished (2 s) | "Finish the path (Enter) or cancel it (Esc) first." |
| History wiped | "History wiped. 142 steps removed." |

The notice is the only announcement; the list is not a live region (criterion 42). The "No undo yet." sentences are deleted (criterion 52); success notices end after the count.

### Wipe, inline confirm (criteria 47 to 51)

The pattern is the design system's "Inline confirm". The footer button "Wipe history" (244 × 28, outline, label `--destructive`) is replaced in place by a block anchored to the footer's bottom edge: the line from criterion 47 (12 px, up to four lines), then **Cancel** (left) and **Wipe** (right), 118 px each, 8 px apart. Focus moves to **Cancel**; Wipe ignores presses for the first 400 ms (a double-click on the old button cannot confirm); Escape, Cancel, a tab change, a selection change or a new step cancels without writing. Wipe: `--destructive` border and label, white ground; pressed or focused it fills with `--destructive` and a white label. The block is a group named by its text (not a dialog). The list above shrinks; nothing floats over content. The button is not in the tree when the list has no step; in a shared document the muted line from criterion 50 stands in its place.

### Empty states

New document: the list area shows "No steps yet." (14 px) and under it, muted, "Steps appear here as you edit." The Now row is still pinned. Wipe button hidden (nothing to wipe). An opened old file whose steps have no time: rows say "Unknown time" in place of the clock.

### Keyboard and accessibility

- List: `role="list"` named "History", rows `role="listitem"` holding a button; **one Tab stop**, roving focus. Up, Down, Home, End, Page Up, Page Down move between rows; Enter or Space presses the row (preview, `0042`); Right moves into the row's action button, Left back to the row; Escape ends a preview, then returns to the canvas. Virtualised rows carry `aria-posinset` and `aria-setsize`; the selected row `aria-current="true"`.
- Accessible name of a row: "Move, 3 paths, You, 12:03, id a1b2c3d, undone" (criterion 42), plus "on branch from a1b2c3d" (`0042`).
- Every status exists as text. Contrast: ink `--toolbar-icon` 8.3:1, muted `--panel-muted-fg` 5.0:1, lane line 3.1:1.

### Scale feel

Rows are 44 px with known positions, so the scrollbar thumb is exact from the first frame. The first 50 rows are drawn at once; rows requested beyond what has arrived show a 44 px placeholder (two muted bars, no spinner) for at most one frame budget. A new step arrives at the top; when the list is scrolled away the scroll position is kept by the height of the inserted row. Scroll position is remembered per scope for the session.

### Edit menu (criterion 8)

Native Edit menu: "Undo Move" / "Redo Move" (plain "Undo", "Redo" when empty, greyed), accelerators Ctrl+Z and Ctrl+Shift+Z; Ctrl+Y is not shown (hidden alias). Then a separator, `0041`'s "Undo Step of Selected Object" and "Redo Step of Selected Object" (Ctrl+U, Ctrl+Shift+U), a separator, "Select All" (Ctrl+A, `0044`). Tooltips name Ctrl+Shift+Z only.

### Criteria changes requested and applied

All changes the ux-engineer requested (criteria 1, 9, 35, 39, 42, 43, 45, 47) were applied to the criteria above by the product owner on 2026-10-10.

### Design questions: decided (defaults, 2026-10-10)

1. Time without a relative form ("12:03" and not "2 min ago"): decided as written (criterion 39).
2. A glyph per kind (five) and not per operation: decided as written.
3. The history list as the one inner scroll area: decided yes (criterion 43); the alternative makes Undo and Wipe unreachable with a long history.
4. Edit menu items for object undo (`0041`) as the fallback for Ctrl+Shift+U: decided yes.
5. Footer label in the Object scope "Wipe object history" (`0041`): decided yes.

## Links

Requirements: R-EDIT-008, R-HIST-001, R-HIST-004
Depends on: `0043-properties-tabs` (milestone 3)
Follow-ups: `0041-object-history`, `0042-history-branches`
ADRs: `adrs.md` (architect); ADR 0014 (accepted 2026-10-10); ADR 0002 §9, ADR 0004 §2, ADR 0009 §1 are the base
PR: -
