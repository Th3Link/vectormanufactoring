# Object history: the steps of one object, object undo and redo, clones that keep their history, wipe

Status: Draft until the customer accepts ADR 0014 (`docs/adr/0014-history-undo-and-branches.md`, Proposed, `needs-customer`). Ready to build on the defaults of ADR 0014 once accepted and once `0020` milestone 2 has merged (milestones 3 and 4 write the optional keys of ADR 0014 §7). `adrs.md` and the UX notes exist; the criteria are complete and testable.
Priority: Should
Origin: Customer (specification of 2026-10-10, final: the history of single objects with undo and redo there, Ctrl+U and Ctrl+Shift+U, an object undo is an editing step that user undo can take back, a clone duplicates the history, a wipe for one object, deletions visible in the global history). Requirements R-HIST-002 and R-HIST-004 (object part). Everything marked **Proposal** is the product owner's idea and is not accepted until the customer says so.

## User value

As a maker I want to select one object and see only its history, take back or bring back its latest step without touching anything else I did since, copy an object together with the story of how it came to be, and clear that story when I no longer want it, so that I can fix one part of a design without undoing the whole session.

**Reference tools.** Inkscape's Undo History is one list for the whole document; there is no per-object history anywhere in Inkscape, LightBurn or Illustrator, to our knowledge. Fusion 360's timeline and Blender's per-object modifier stack are the nearest ideas. The customer says he has rarely seen "an undo that is itself an undoable step" outside git and has often missed it. That is the git `revert` idea: taking something back is a new, visible step, and taking that back is a step again. What we do better: the object's list shows who did what to this object and when, the clone keeps its lineage, and Ctrl+Z, Ctrl+U and the list all agree.

## Words used below

All terms of `0020-undo-redo` (step, undo stack, global history, operation name, step id, touched objects, kind). In addition:

- **Object timeline:** the steps that touched one object, oldest to newest. A step that touched three objects is in three timelines.
- **Do step:** a step an interaction made (Move, Union, Change style). **Revert step:** a step made by an object undo (criterion 8); it is also a row in the timeline.
- **Object undo:** take back the object's latest applied do step, for this object only. **Object redo:** bring back the one object undo took back last.
- **Inherited rows:** the rows a clone takes over from its original (criterion 24).
- **Floor:** the point before which an object's history was wiped (criterion 29).
- **Clone:** a copy of an object made by Duplicate or by Ctrl-copy; the word the customer uses. Elsewhere in the product the command is "Duplicate".

## Acceptance criteria

### The object scope of the History tab

1. Given the History tab (`0020` criterion 39) and exactly one object selected, then the tab has a scope group **Document / Object** (an inline segmented group). Object is pressed by default. The list in the Object scope shows the object's timeline: its do steps, its revert steps and its inherited rows, newest at the top, each row as in `0020` criterion 39 plus the status (Applied or Undone). Steps before the object's floor are wiped (criterion 30) and are not listed, so a "wiped" status is never shown. Given no object or more than one selected, then the group is not in the tree and the list is the document list. When the selection becomes exactly one object, the scope becomes Object. A press on Document is kept while the selection stays one object, also when it moves to another single object, and leaving the single-object state resets it to Object. The scroll position of each scope is kept for the session; a change of scope ends a preview (`0042`).
2. Given the Object scope, then the subject line in the strip row of the tab header (`0043`) names the object by kind and the first 6 characters of its id ("Rectangle 3f9a1c", "Compound path 3f9a1c"); the kind may shorten with an ellipsis, the id never. The tooltip has the kind and the full id, as selectable text. In the Document scope the subject line reads "24 steps". (Objects have no names yet; layers and names come with `0039`.)
3. Given a step that also touched other objects, then its row in this object's list says so: "Move, with 2 other objects". Given the step id, then it is the same id as in the document list.
4. Given a deleted object, then it has no object scope (it cannot be selected). Its deletion is a row in the document scope (`0020` criterion 38) that names the kind, and `0042` offers a clone of its last state.

### Object undo and redo

5. Given exactly one object selected, the Select tool or the Node tool active, and the focus not in a text field, when the maker presses Ctrl+U (Cmd+U on macOS), then the object's latest applied do step is taken back for this object; Ctrl+Shift+U (Cmd+Shift+U) brings back the one object undo took back last. The keys are gated exactly like `0020` criterion 6 (no effect during a drag, with a chip open, in a text field, during a long operation, with an unfinished Pen path), are matched by the typed character (QWERTZ, `0020` criterion 7), and auto-repeat acts once per event. With none or several objects selected the key does nothing and the notice says "Select one object." The native Edit menu (`0020` criterion 8) has the two items "Undo Step of Selected Object" and "Redo Step of Selected Object" with Ctrl+U and Ctrl+Shift+U, after Redo and set off by separators, greyed unless exactly one object is selected; pressing one is the same as the key. They are the working route if a platform eats Ctrl+Shift+U (Question 5).
6. Given an object undo, then **only this object's part** of the step is taken back, depending on the step's kind (`0020` criterion 32):
   - *Change* (a move of three objects, a style edit of three): this object's fields go back to their values before the step; the other objects stay as they are. Test: move A, B, C in one drag, select B, Ctrl+U: B is back, A and C stay moved.
   - *Create only* (a Duplicate of three objects): this object is removed; the others stay.
   - *Replace* (a Boolean, Combine, Break apart, Group, Ungroup and the like: objects deleted and created together): the **whole step** is taken back, because taking back only the result would lose the operands. The notice names the others: "Undid: Union. 2 other objects came back too."
7. Given the same field conflict as `0020` criterion 16, then an object undo writes a field back only if its current value is still the value the step wrote. A later change by anyone keeps its value; the notice says "Undid: <name>. N fields kept later changes."
8. **An object undo is a step.** Given an object undo, then it is a new step authored by me, with the operation name "Undo <operation name>" (kind *Object undo*), on my undo stack, in the global history and in the object's timeline. Its row says "reverts a1b2c3d" (the step it took back); it is one commit whose header names the reverted step and the object (ADR 0014 §2). The step it reverted stays in the list with the status word "Undone by You" (another author: "Undone by <author>"). The wording follows `0020` criterion 39: plain "Undone" is only for a step I took back with Ctrl+Z; anything else is "Undone by <author>".
9. **User undo of an object undo is an object redo.** Given steps 1 (move A), 2 (move B), then an object undo of step 1 (step 3), when I press Ctrl+Z, then step 3 is taken back, so A is moved again; a second Ctrl+Z takes back step 2 (B), a third step 1 (A). The stack is plain last-in-first-out; nothing special is needed. Test with `document.json` equality at each point.
10. Given an object redo, then it is a new step "Redo <operation name>" (kind *Object redo*), and Ctrl+Z takes it back, which restores the object undo's state. Test: Ctrl+U, Ctrl+Shift+U, Ctrl+Z gives the state after Ctrl+U.
11. Given an object undo and then a new do step on the same object (by anyone), then there is nothing to redo for that object: Ctrl+Shift+U says "Nothing to redo for this object." The steps taken back stay in the list; `0042` shows them as a branch and lets the maker return to them.
12. Given the object's first step (its creation), when it is taken back by an object undo, then the object is removed, the selection is empty and the notice says "Undid: Draw rectangle. The object is gone. Ctrl+Z brings it back." Ctrl+Z restores it with the same id and the same history (`0020` notes: ids survive an undo of a delete).
13. **Who can be taken back.** Given the object's latest do step was made by another peer, then (**Proposal, Question 1**) the object undo takes it back and the notice names the author: "Undid Anna's Resize (a1b2c3d)." The author is also in the tooltip of the row. The other peer's own Ctrl+Z is unaffected and skips the fields that changed since (`0020` criterion 16). Alternative: refused with "That step is by someone else."
14. Given an object undo that finds nothing to take back, then the notice says "Nothing to undo for this object." and nothing is written.
15. Given an object undo or redo, then the selection stays on the object when it still exists, and is empty when the step removed it (criterion 12). The tool does not change; pan and zoom do not change.
16. Given the Object scope, then its header has two icon buttons **Undo step** and **Redo step** in the same two reserved 28 px slots as the Document scope (`0020` criterion 9), present only while the action is possible, so the one that stays does not move; tooltips name the step: "Undo: Resize, You, 12:03 (Ctrl+U)". Pressing one is the same as the key.
17. Given each object undo or redo, then the time and memory budgets of `0020` criteria 29 and 30 apply to it.

### Groups, compound paths, steps with many objects

18. Given a group (`0023`), then its timeline is the steps that touched the group node **or any object below it**, so moving the group, restyling the group (which restyles every leaf), and an edit of one leaf all show in the group's list. An object undo of a group takes back the latest such step **restricted to the group's subtree** (criterion 6). A leaf inside the group has its own timeline, shown when it is selected in the entered group. Grouping and Ungrouping are *Replace* steps and show in the timelines of every child.
19. Given a compound path (`0016`), then it is one object with one timeline. Combine and Break apart are *Replace* steps: the pieces' timelines end with "Deleted by Combine" and the result's begins with "Created by Combine from 3 objects"; Break apart the other way.
20. Given a Boolean over A, B, C, then the result's timeline begins with "Created by Union from 3 objects" and lists the source ids in the tooltip; the sources' timelines end with that step as *Delete*. The result does not inherit a source's history (**Proposal**, Question 2: the alternative is that the result inherits the bottom operand's history, as it keeps its style).
21. Given a step that touched N objects, then it appears in N timelines (test: a move of 3 objects is a row in each of the three) and once in the document list.
22. Given nodes, anchors and handles, then they are not objects: a node edit is a *Change* step of its path and appears in the path's timeline as "Move nodes". There is no per-node history.

### A clone keeps the history

23. Given Duplicate or the Ctrl-copy move of objects, then each copy (clone) is a new object with a new id whose timeline begins with the row "Duplicate of 3f9a1c".
24. Given the clone's timeline, then below that row it shows the **inherited rows**: the original's rows up to the moment of duplication, flagged "from 3f9a1c", with the same step ids and operation names. (The ids are the same because they are the same steps.) Inherited rows are a copy: later steps of the original never appear in the clone's timeline, and later steps of the clone never appear in the original's. Test: draw, resize, duplicate, move the original, move the clone: each timeline has exactly its own move.
25. Given an object undo on a clone, then it can go back **into the inherited rows**: it sets the clone's fields to the state they had at that earlier point of the lineage, as a new step on the clone. It never changes the original. (The architect: this is a restore of state, not an inverted operation, because the operations belong to the original.)
26. Given a clone of a clone, then the lineage goes back through both, up to the floor (criterion 29) of whichever was wiped.
27. Given a duplicate of a group, then every descendant is a clone with its own inherited rows and the new group gets one too.
28. Given a Boolean, Combine, Break apart, Offset, or any command that makes objects from objects, then the new objects are **not** clones and inherit nothing (criterion 20).

### Wipe the history of one object

29. Given the Object scope and at least one step before the floor, then the last element (the footer) is a control labelled **Wipe object history** (in the Document scope the label is "Wipe history": the two do different things, one hides and one removes data). Given nothing would be hidden (no step before the floor), then the control is not in the tree. Pressing it replaces the control in place, in the panel (no popup), by the line "Hide the N earlier steps of this object? They stay in the document history. The file does not get smaller." with the buttons "Cancel" (left) and "Wipe" (right). Keyboard focus goes to Cancel; "Wipe" ignores presses for the first 400 ms; Escape, Cancel, a change of tab, of selection or a new step cancel without writing. (Question 3 for the wording and the meaning.)
30. Given "Wipe", then a step "Wipe history" (kind *Document*, touched object: this one) is written; the object's timeline starts with the row "History wiped" and shows only later steps; inherited rows and earlier steps are gone from the object scope; an object undo cannot go before the floor; a clone made later inherits only from the floor. The step is not undoable: it is not on my stack, and Ctrl+Z skips it.
31. Given a wipe, then my undo and redo stacks lose the entries that touched **only** this object; entries that also touched other objects stay and still take back their effect on all their objects. Test: A moved alone, A and B moved together, wipe A: Ctrl+Z takes back the pair, the next Ctrl+Z says "Nothing to undo."
32. Given a wipe, then the document list still shows every earlier step (a wipe hides the object's story, it does not delete data), plus the row "Wipe history of Rectangle, 3f9a1c". Wiping the history of the whole document removes data: `0020` criteria 47 to 51.
33. Given a wiped object, then the wipe is saved: after Save and Open the object's timeline still starts at the floor. A wipe never changes a clone made earlier: the clone keeps its copy of the inherited rows (criterion 24), and wiping the clone leaves the original's timeline unchanged.
34. Given a drag, a chip or an unfinished Pen path, then "Wipe history" is ignored, like `0020` criterion 6. Given a document that has been shared, then (**Proposal**) a wipe is offered as in a single-user document, because it only hides: it is a step that replicates and every peer's list honours it.

### Cost

35. Given a timeline, then the Object scope opens within 100 ms for an object with 1,000 steps in a document with 20,000 steps. The index of steps per object is built when the file is opened or the first time it is asked for, not on every commit slower than 1 ms per commit.

### What is stored (ADR 0014, Proposed; testable on its defaults)

36. **A timeline is derived.** Given an object, then its timeline is computed from the step log by filtering the steps that touched it; the per-object index is built in one pass on the first request of the Object scope and then extended by one append per commit and per import. Nothing about the timeline is stored except the two optional keys of criteria 37 and 38. A step that touched N objects is in N timelines (criterion 21).
37. **Clone origin.** Given Duplicate or the Ctrl-copy move, then each copy gets the optional key `clone_of` = `<source id>@<version>` (the source's `NodeId` and the document version at the duplication), written in the same commit as the copy. A copy never takes over the source's `history_floor` or `clone_of`: a clone of a clone points at its direct source, and criterion 26 follows the chain. Inherited rows (criterion 24) are computed on request from the source's steps in the causal past of that version and are never copied into the clone, so later steps of either object cannot appear in the other. A value that cannot be read means "no origin" and never makes the file unreadable.
38. **Floor.** Given a wipe (criterion 30), then the step writes the optional key `history_floor` on the object = the document version at the wipe. The timeline hides every step in the causal past of the floor; a step by a peer that was concurrent with the wipe stays visible. The key is the one write of the wipe step, so a wipe is one commit and one step.
39. **No format bump.** Given a file with `clone_of` or `history_floor`, then `format_version` is unchanged (ADR 0014 Q2 A, the default). A build that does not know the keys ignores them and draws the same drawing; an older build that duplicates an object copies `clone_of` as it is, which shows a wrong lineage in a newer build and never a wrong drawing.
40. **Object undo is a restore, not an inverse.** Given an object undo, a go to version or a clone, then the engine reads the object at the step's before and after versions and writes fields (criterion 7); this is the same engine as `0020` and has no stored inverse. Given an object undo of a step that deleted the object, then the object comes back with the same id (`0020` criterion 17 and the spike S1 of ADR 0014).
41. **Latest applied and redoable come from the stack machine.** Given an object's timeline, then which step Ctrl+U takes back and which step Ctrl+Shift+U brings back are the output of the one stack machine of `0020` criterion 55, run over this object's timeline with the object-undo and object-redo marks. A new do step on the object clears its redo side (criterion 11). With ADR 0014 Q6 B (own steps only) the machine runs over the current peer's steps of the object.

## Out of scope

- Taking back an arbitrary older step of an object, previewing a version, going to a version, branches, clone of an earlier state, restoring a deleted object: `0042-history-branches`. Object undo here moves one step at a time.
- Per-object history for several selected objects at once.
- Names for objects and layers (`0039`).
- Per-node or per-anchor history.
- Removing an object's data from the file (the document wipe of `0020` does that).
- Comparing two versions, a diff view, thumbnails per row.
- A history for settings that are not objects (document size, unit, background): they are in the document list only.
- Linked clones (Inkscape's Alt+D "Clone"): not what the customer means.
- Selecting the objects of a row by clicking it (the click is `0042`'s).

## Notes for the architect (answered by ADR 0014 and `adrs.md`)

- The per-object index, the revert mark, the clone origin and the floor are decided: a derived index (criterion 36), step headers (ADR 0014 §2), two optional keys `clone_of` and `history_floor` that older builds ignore, no `format_version` bump by default (ADR 0014 §7, Q2 A). They are the document-model change of this spec; the architect reviews the branch diff at milestone 3.
- State at a version is a read of the live document at a past version, then a write as one commit (ADR 0014 §1); an inverted operation cannot serve the inherited rows, because the original's operations are on another container.
- Ctrl+Shift+U on Linux is the GTK and IBus chord for Unicode input in text widgets. Verify by hand in milestone 2 that it reaches the page on WebKitGTK; the Edit menu items of criterion 5 are the fallback.

## Open questions (customer; each has a default)

1. **Whose steps can an object undo take back?** *A (default, recommended):* any author's latest step on the object, with the author named in the notice and the row, and itself undoable by Ctrl+Z (this is ADR 0009 option C, which that ADR deferred to a history view). *B:* only my own steps; the others are listed but cannot be taken back. A fits the customer's "go into the history and do undo and redo there"; B is the safe choice in a shared document. In a single-user file there is no difference.
2. **Does the result of a Boolean (or Combine) inherit a history?** *A (default):* no, it starts with "Created by Union from 3 objects". *B:* it inherits the bottom operand's history, because it keeps that object's look. *C:* it inherits all operands' histories side by side. Recommendation A: B and C make "whose story is this?" unclear.
3. **What a wipe of one object means.** *A (default, recommended):* it hides and cuts: the earlier steps stay in the file and in the document list, only the object's own list, object undo and later clones start at the floor; the confirmation line says so. *B:* no object wipe; only the document wipe removes data. *C:* try to remove the object's operations from the file: not possible in a replicated log without rebuilding the document, which would break every peer. The customer asked for the switch; A is the honest version of it.
4. **Default scope (criterion 1).** *A (default):* Object scope when exactly one object is selected. *B:* always Document scope, the maker switches. Recommendation A.
5. **Ctrl+U and Ctrl+Shift+U (criterion 5).** Ctrl+U is "view source" in desktop browsers and Ctrl+Shift+U starts Unicode entry in GTK text widgets. In the desktop web view neither is a problem unless the test shows otherwise. *Default:* the customer's keys. *Fallback if a platform eats a key:* the buttons of criterion 16 stay, and the architect names an alternative with the customer.
6. **History of a group (criterion 18).** *A (default):* the group's list includes steps on any descendant. *B:* only steps on the group node itself (group, ungroup, reorder). Recommendation A: moving or styling a group is what the maker does.

### The six questions of ADR 0014 (architect, `needs-customer`)

ADR 0014 is `Proposed`; the defaults below are the architect's recommendations and apply when the customer does not answer. The same six questions stand in `0020` (Questions 11 to 16) and `0042`.

7. **History in the file (ADR 0014 Q1).** *A (default):* keep it (it is there today), the History tab says so, a wipe is available. *B:* strip the history on every Save.
8. **File-format footprint (ADR 0014 Q2).** *A (default):* no `format_version` bump; step headers in commit messages and three optional keys (`history_wiped`, `history_floor`, `clone_of`) that older builds ignore. *B:* bump the version at this spec, so older builds refuse files that carry lineage keys.
9. **Wipe in a shared document (ADR 0014 Q3).** *A (default):* the document wipe is not offered until sharing exists (the object wipe of criterion 34 only hides and stays). *B:* admins only, with a new key epoch and a fresh sealed snapshot (ADR 0010 §8); copies other people already hold keep the old history. *C:* no document wipe at all.
10. **Object wipe (ADR 0014 Q4; Question 3 above).** *A (default):* it hides and cuts, the data stays in the file, the confirmation says so. *B:* no object wipe.
11. **Authors (ADR 0014 Q5).** *A (default):* a random id per session, shown as "You" and "Earlier session". *B:* one random id per installation in every step, so "You" survives reopening, but every file you share carries the same identifier. *C:* the OS user name in the file.
12. **Taking back someone else's step (ADR 0014 Q6; Question 1 above).** With Ctrl+U or go to version, never with Ctrl+Z (ADR 0009 option C). *A (default):* allowed, as a named step that Ctrl+Z takes back. *B:* own steps only.

## UX notes

Written by the ux-engineer, 2026-10-10. Row anatomy, notice slot, inline confirm and lane drawing are in `0020` (UX notes) and `docs/design-system.md`; this section says what the Object scope adds. The two scopes are **one list component** with two data sources.

### Scope group and subject line

- **Scope group:** a `ToggleGroup` with text items "Document" (76 px) and "Object" (56 px), 28 px high, left in the header row of the History tab, `role="radiogroup"` named "History scope", one Tab stop, arrows move and select. It is in the tree only while exactly one object is selected; the slot stays empty otherwise, so Undo and Redo on the right do not move.
- **Default and stickiness:** when the selection becomes exactly one object (from none or from several) the scope becomes Object (Question 4 default). A press on Document is kept while the selection stays one object, also when it moves to another single object; leaving the single-object state resets it to Object for the next time. The scroll position of each scope is remembered for the session. A change of scope ends a preview.
- **Subject line** (strip row, right-aligned, 120 px): kind in ellipsis-able text plus the 6-character id as fixed text: "Rectangle 3f9a1c", "Compound path 3f9a1c" (the kind shortens, the id never). Tooltip: kind and the full id, selectable. Document scope: "24 steps".

### Rows that exist only here

| Row | Look |
|---|---|
| A do step | As `0020`. The summary slot of line 2 reads "with 2 other objects" when the step touched more than this object |
| An object undo | Glyph `Undo2`, name "Undo Resize", line 2: "reverts a1b2c3d · You · 12:05". The reverted step stays in the list with the word "Undone by You" (when the author is somebody else, "Undone by Anna") |
| An object redo | Glyph `Redo2`, name "Redo Resize", line 2 "restores a1b2c3d · …" |
| Created from objects | Glyph `Plus`, name "Created by Union", summary "from 3 objects"; the tooltip lists the source ids (selectable). The sources' timelines end in "Deleted by Union", glyph `Minus` |
| Clone start | Glyph `Copy`, name "Duplicate of 3f9a1c", line 2 "You · 12:04". It is the boundary between the clone's own rows and the inherited ones |
| Inherited rows | Below the clone-start row, in the same ink as any applied row (not dimmed: they are history, not undone steps). Line 2 starts with the word "from 3f9a1c" in semibold; the lane segment between them is **dashed** and the marks stay filled discs. Undone is a ring and dotted: the two never look alike. Their row action does not exist (they belong to the original) |
| Floor | The last row of a wiped timeline: glyph `Eraser`, name "History wiped", line 2 "Earlier steps hidden · You · 12:10". Nothing is listed below it |

Wiped rows leave the list. They have no status word because they cannot be seen (see the change request for criterion 1).

### Object undo and redo

- **Buttons:** the same two reserved slots as the Document scope, now acting on the object (criterion 16). Glyphs stay `Undo2` and `Redo2`; what tells them apart is the pressed "Object" item next to them and the tooltip: "Undo: Resize, You, 12:03 (Ctrl+U)", "Redo: Resize (Ctrl+Shift+U)". They leave the tree when the action is not possible.
- **Feedback for the keys:** the notice in the canvas slot, the selection stays, the new row "Undo Resize" appears at the top of the timeline in the same frame (when the History tab is shown), and the Now row's id changes.

| Event | Notice |
|---|---|
| Change step, own | "Undid: Resize." |
| Replace step | "Undid: Union. 2 other objects came back too." |
| Field conflict | "Undid: Resize. 1 field kept later changes." (N fields: "N fields kept later changes.") |
| Another author's step | "Undid Anna's Resize (a1b2c3d)." Until names exist: "Undid a step by someone else: Resize (a1b2c3d)." |
| Creation step | "Undid: Draw rectangle. The object is gone. Ctrl+Z brings it back." (5 s, longer than 70 characters) |
| Redo | "Redid: Resize." |
| Nothing | "Nothing to undo for this object." / "Nothing to redo for this object." |
| Not one object | "Select one object." |

The keys, gate and repeat follow `0020` criterion 6. The Edit menu gets two items after Redo, separated: **"Undo Step of Selected Object"** and **"Redo Step of Selected Object"** with Ctrl+U and Ctrl+Shift+U, greyed unless exactly one object is selected. This costs nothing, shows the keys, and is the working route if a platform eats Ctrl+Shift+U (Question 5).

### Wipe of one object

The footer button reads **"Wipe object history"** in this scope and "Wipe history" in the Document scope. The two do different things (one hides, one deletes data), and the label is the only thing the maker sees before pressing. The inline confirm is the one of `0020` with the line of criterion 29; Cancel has focus, Wipe ignores presses for 400 ms. After Wipe: the notice "Hid 12 earlier steps of this object.", the timeline starts at the floor row. The button is not in the tree when the timeline has no step before the floor.

### Empty and rare states

- Object whose timeline the file does not hold (very old file): one row "No steps recorded for this object." (muted, not a step); Wipe not in the tree.
- A group: kind "Group"; its rows include steps on any descendant, written as the step's own name (criterion 18); the summary slot says "in 2 objects of the group" when the step touched leaves below it.

### Criteria changes requested and applied

All changes the ux-engineer requested (criteria 1, 2, 5, 8, 16, 29) were applied to the criteria above by the product owner on 2026-10-10. "Wiped" is no longer a status (here and in `0042` criterion 4).

### Design questions: decided (2026-10-10)

1. Edit menu items for object undo: decided yes (criterion 5); they are the fallback for Ctrl+Shift+U on Linux.
2. Footer label "Wipe object history" in the Object scope, "Wipe history" in the Document scope: decided yes (criterion 29).

## Links

Requirements: R-HIST-002, R-HIST-004
Depends on: `specs/0020-undo-redo/`, `specs/0043-properties-tabs/`
Follow-up: `specs/0042-history-branches/`
Related: `specs/0023-groups/` (criterion 18), `specs/0016-boolean-operations/`, ADR 0009 §1 (option C)
ADRs: `adrs.md` (architect); ADR 0014 (Proposed, `needs-customer`: the floor and origin keys, the object wipe, taking back other people's steps)
PR: -
