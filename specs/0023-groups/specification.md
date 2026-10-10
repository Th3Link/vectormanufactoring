# Groups: group, nest, enter and leave, ungroup

Status: Ready (2026-10-10: `adrs.md` and the UX notes exist; the customer accepted the document-model change on 2026-10-10 (Question 1: yes); criterion 27 reworded the same day)
Priority: Must
Origin: Customer (groups, nested groups, entering a group, ungrouping; request of 2026-10-09, with the wish that the UX be much better than Inkscape's). Requirement R-EDIT-009 (grouping and layers) is in the customer-confirmed MVP; this spec is its **grouping half**. Layers are a separate, later entry (`0039`, see "Groups and layers"). The decisions marked in "Decided by the product owner" are my proposals.

This spec takes the number 0023 that was reserved for the MVP slice `layers-and-grouping`. Grouping is built first; the layers half keeps the slice label 10 in the list of `specs/README.md` under the new number 0039.

## User value

As a maker I want to bind several parts into one group, move, scale and rotate it as one thing, put groups inside groups, open a group to work on one part in it, and take it apart again, so that an assembled design (a box with its lid, a sign with its letters) stays organised and I can still reach every part without ungrouping.

**Where Inkscape's grouping is weak, and what we do instead** (from its manual and from how its groups behave; the Inkscape-specific statements are the kind to check against the version the customer uses):

- *No sign that you are inside a group.* Inkscape makes a group you double-click the current layer; the only sign is a small name in the status bar. The rest of the drawing stays fully drawn and looks editable. Here, everything outside the open group is dimmed to 35 % and cannot be hit, and a visible indicator names the open group with a way out (criteria 15 to 18).
- *Hidden gestures to reach into a group* (Ctrl+click, Ctrl+Alt+click, double-click, a layer drop-down). Here: click selects the group, double-click opens it, Escape closes it. One gesture each, and each is shown in a hint.
- *Group transforms that pile up.* A group in Inkscape carries a transform that accumulates and is baked on ungroup (sometimes changing stroke widths). Here a group has **no transform and no style of its own** (criteria 22 and 25): moving or rotating a group changes the geometry of its parts, so ungroup never changes how anything looks.
- *Commands only in menus.* Group and Ungroup are Object-menu items. Here they are buttons the maker can see (placement: UX), and the tooltip says what they do.

## Words used below

- **Group:** an object with an ordered list of children (objects or groups). It has no geometry, no style and no transform of its own. Its place among its siblings is its z-order.
- **Leaf:** an object that is not a group (path, compound path, rectangle, ellipse, polygon, star).
- **Context:** the level the Select tool works on: the document root, or the group that has been entered. Selection, marquee, Alt-click cycling and "select all" see only the **top-level objects of the context** (its direct children).
- **Entered group:** the group that is the context. Nested levels are a stack: root, group, group inside it, and so on.

## Acceptance criteria

### Group and Ungroup

1. Given the Select tool, a context and two or more objects selected (any mix of leaves and groups, all children of the context), when the maker activates **Group**, then one new group is created that holds the selected objects as its children in their existing relative order, the group sits in the context at the stacking position of the topmost selected object, and it is the only selected object. Objects that were between the selected ones and not selected keep their order and end up below the group. One commit, stored as `group`. Test: A (bottom), B, C (top) with A and C selected: the order becomes B, group(A, C).
2. Given fewer than two objects selected, then Group is not offered as an available command (dimmed in the rail, see "Where the commands live") and does nothing. Given a selection that includes a compound path or a primitive, then it is grouped like any other leaf.
3. Given one or more groups selected, when the maker activates **Ungroup**, then each selected group is replaced, at its stacking position, by its children in their order, and the children are selected together with any other objects that were selected. Nested groups inside it stay groups (one level per Ungroup). The objects look exactly as before: no geometry, style or position of any child changes. One commit, stored as `ungroup`. Given the selection holds no group, Ungroup is not available and does nothing.
4. Given Group then Ungroup on the same selection with no non-selected object between the selected ones, then the document is as before apart from the new object ids of the groups. With non-selected objects between them it differs by the order of criterion 1, which is shown in the Group tooltip.
5. Given a group is nested inside another group, then depth up to 32 levels is allowed. Given a Group that would exceed it, then it is refused with the notice "Groups can be nested 32 levels deep. Nothing was changed." and nothing changes.
6. Given a successful Group or Ungroup, then a one-line notice names what happened ("Grouped 3 objects." or "Ungrouped 2 groups into 7 objects."), ends with "No undo yet." until `0020-undo-redo` exists, disappears within 3 seconds, takes no focus and is a `role="status"` live region, as `0016` criterion 29.
7. Given the shortcuts, then Group is **Ctrl+G** and Ungroup is **Ctrl+Shift+G** (Cmd on macOS), which Inkscape and Illustrator share. They are gated like every shortcut (not during a drag, with a chip open or in the Pen with an unfinished path, `0010` criterion 55). Proposal (Question 4): they are a pair of undoable-by-each-other commands, unlike the Boolean operations that had no shortcut because they cannot be undone.

### Selecting groups

8. Given a context and a click on a leaf that is inside a group of that context (hit by the Select tool's 8 px outline rule or its filled interior, `0014` criterion 1, `0007` criterion 29), then the **top-level object of the context that contains it** (the group) is selected, not the leaf. A click on a leaf that is a direct child of the context selects the leaf, as today.
9. Given a marquee or lasso selection, then it chooses among the top-level objects of the context only. A group counts as one object whose box is the bounding box of all its leaves' outlines (the group box of `0019-multi-object-transform` criterion 1), and the touch and enclose rules of `0014` criteria 9 and 10 apply to that box. Given Shift-click, then it toggles a top-level object in or out of the selection.
10. Given the Alt-click cycle of `0014` criterion 4, then its candidates are the top-level objects of the context whose leaves lie within tolerance of the point.
11. Given a group selected, then the Properties panel shows the Style area with the subject line "Group of 5 objects" (the number of leaves, nested groups counted through) or, for several selected groups, "2 groups" (a mix: "4 objects"). The texts follow `0017-style-panel-rework` criterion 4.
12. Given a group selected and the Node tool active, then the nodes of every path leaf in the group, nested groups included, are shown and editable as if those paths were selected one by one (`0006` criterion 6). Compound paths contribute no nodes (`0016` criterion 38a).
13. Given Delete, Duplicate (and the Ctrl-copy move of `0010`) on a selection with a group, then the whole group with all its descendants is deleted or copied. A copy gets new ids for the group and for every descendant and every anchor (`0016` criterion 36a).
14. Given a group loses its last child (the child is deleted or the Node tool's last-node delete removes the object), then the empty group is removed in the same commit. A group of one child is allowed. If the group was the entered group, the context moves to its parent.

### Entering and leaving a group

15. Given the Select tool and a double-click on a group of the context (or the Enter key with exactly one group selected), then the group is entered: it becomes the context, the double-clicked leaf (or the top-level object of the new context that contains the point) becomes the selection, or the selection is empty when Enter was used. Everything outside the entered group is drawn at 35 % of its normal strength, cannot be hit, selected, marqueed or moved, and does not take part in the Alt-click cycle. The entered group's own content is drawn normally. Given a double-click on a leaf path that is a direct child of the context, then the Node tool opens as today (`0009`); on a primitive nothing changes.
16. Given a group is entered, then an indicator on the canvas names the stack of entered groups (for example "Group" or "Group > Group") and each level in it is a pointer target that makes that level the context (a click on "Group" leaves the deeper levels; a root entry leaves everything). The indicator is a readout with buttons, not a popup; its look and position are the ux-engineer's. It is absent at the root.
17. Given the Select tool, a group entered and nothing selected, when the maker presses Escape, then the context moves up one level and the previous level's group is selected. With a selection, Escape clears it first (`0010` criterion 42, the Select tool's step 3). This adds one step to the Escape cascade of `0010` criterion 42: in the Select tool, clearing the selection comes first, leaving a group second, and the empty Select tool does nothing more (it is the last tool in the cascade).
18. Given a group entered, when the maker double-clicks empty canvas (no object of the context within tolerance), then the context moves up one level. A single click on empty canvas clears the selection, as today.
19. Given a group entered, then every tool works in that context: objects drawn with the Pen or a shape tool, pasted, or created by an operation become children of the entered group, on top of its other children. Boolean results, Break apart and similar results take the place of their base operand inside the entered group (`0016` criterion 22).
20. Given the entered group is selected through another route (the document is reopened, the group is deleted, an operation removes it), then the context falls back to the nearest ancestor that still exists. The entered state is a view state: it is not saved in the file, and a reopened project starts at the root.
21. Given a group entered and a pan, zoom or tool change, then the context and the dimming stay.

### Transform, style and the model

22. Given a group selected, then it has the group box and the handles of `0019-multi-object-transform` (move, resize, rotate, centre handle, skew when every leaf is a path, typed values), computed over all leaf outlines. A transform is applied to every leaf, exactly as it would be for a multi-selection of those leaves (`0019`). The group does not remember a rotation: after a rotate the box is again the axis-aligned bounds of the leaves (the answer to the "persistent orientation" question: none in this version, Question 2). A single group selected with the Select tool shows only the group box, not one box per leaf.
23. Given a transform of a group, then it is one commit (`0019` criteria for the commit) and no leaf is restyled, and a "Scale stroke width" off setting leaves all stroke widths as they are.
24. Given Fit to content (`0015`), the rulers and the status bar, then groups are transparent: the bounds are those of all leaves.
25. Given the document model, then a group is a node of the document tree (ADR 0002 §5: a tree whose sibling order is z-order) with no style register and no transform register: a style edit on a group is an edit of every leaf below it (criterion 26), and there is no style inheritance. The architect confirms in `adrs.md`; a group node that carries a transform is a later extension (Question 2). **This is a document-model and file-format change (`CLAUDE.md` §3). The customer decided yes on 2026-10-10 (Question 1).**
26. Given a style edit (stroke, fill, dash, markers, opacity) with a group selected, then it applies to every leaf descendant, recursively, in one commit. A value that differs between leaves shows "Mixed" (`0017` criteria 9 and 14). A leaf that cannot have the style (a marker on a compound path, `0016` criterion 38b) is skipped, as in a multi-selection.
27. Given a project saved with groups and reopened, then the tree, the order, every leaf and its style are as before. `format_version` goes to the next free number at merge (`specs/README.md`, "`format_version` plan"); a file with a group opened by an earlier build is refused with the "saved by a newer version" message; a file from an earlier build opens unchanged. A file with a group nested deeper than 32 is refused as damaged and does not crash. (A cycle cannot occur: the object tree of the file format cannot hold one, so only the depth limit is tested; reworded 2026-10-10, `adrs.md`.)

### Interplay with the operations of other specs

28. Given Boolean operations (`0016`), Combine, Break apart, Split, Cut, Fracture, Flatten and Offset (`0035`, `0048`, `0036`, `0037`, `0038`) and a selection that contains a group, then the command is refused with "<Operation> does not work on groups. Ungroup first. Nothing was changed." (the refusal style of `0016` criterion 15). A later spec may let them look through groups; this one does not.
29. Given Object to path with a group selected, then it is not offered (Question 5).
30. Given the Pen's continue and connect targets (`0034-pen-path-extension`), then only open paths that are children of the context are targets.

## Where the commands live

Group and Ungroup are buttons, not menu-only items: a card of two buttons, the second card of column A of the left tool rail, below the Tools card, always rendered and dimmed when they do not apply, as the Boolean buttons are. The rail layout (two columns, which card sits where, why) is decided in `docs/design-system.md`, row "Tool rail architecture"; the UX notes below give the Group card.

## Groups and layers

Layers (named, with visible and locked flags, MVP slice 10, R-EDIT-009) are a separate entry, `0039-layers`, not yet specified. They are designed to follow from groups: a layer is a top-level group that has a name and two flags. This spec stores no name and no flag on a group, so it neither helps nor blocks that: the model of criterion 25 can take extra registers later without changing any criterion here.

## Out of scope

- **Layers**, visibility and lock, named groups, an Objects panel with a tree (`0039`).
- **Moving objects into or out of a group** without ungrouping (drag into a group, "Move to group", Inkscape's "Move to layer"). The way to add an object to a group today is to enter it and draw, or to ungroup and group again (Question 3).
- **A group transform or a persistent group box orientation** (Question 2).
- **Group-level style inheritance**, clip paths, masks, opacity of a group as a whole, blend modes.
- **Boolean and other path operations through groups** (criterion 28).
- **Object to path on a group** and "Ungroup all" (recursive) as a separate command.
- **Selecting inside a group without entering it** (Ctrl+click in Inkscape and Illustrator). Question 6.
- **Isolation of an object other than a group**, locking or hiding while entered.
- **SVG export and import** of groups (`svg-import-export`, `<g>` maps naturally; this spec defines nothing for it).
- **Undo and redo** (`0020`). Each command is one commit.

## Open questions

Each has a default; nothing blocks. The document-model question is decided (yes, 2026-10-10).

1. **Document model (criteria 25, 27).** A group as a tree node with children, no style, no transform, a `format_version` bump. **Decided: yes (customer, 2026-10-10).** The ADR in `adrs.md` records it. The alternative was no groups.
2. **Group transform (criterion 22).** *A (default):* none; transforms are applied to the leaves and the box is axis-aligned, as `0019` does for a multi-selection. *B:* a group keeps a rotation register so its box stays oriented like a single rotated object's (`0005`), which needs a transform on the group node (ADR 0002 §5 allows it) and every leaf's geometry to be resolved through the chain. B is a larger model change and is the source of the "baked transform" confusion in Inkscape. Recommendation: A.
3. **Moving into and out of groups.** Default: not in this slice. Option: a "Move out of group" command (one level up) and drag-and-drop into a group in a later Objects panel.
4. **Shortcuts (criterion 7).** Default: Ctrl+G and Ctrl+Shift+G are in. Option: none, like the Booleans (`0016` Question 3).
5. **Object to path on a group.** Default: not offered. Option: converts every primitive inside.
6. **Reaching inside without entering.** Default: no (double-click enters). Option: Ctrl+click selects the leaf inside a group, as Inkscape and Illustrator do. `0014` gives Ctrl a meaning in a marquee and in a move but none in a click, so the key is free; I left it out because the customer wants fewer hidden gestures.
7. **Dimming strength (criterion 15).** Default 35 %. The ux-engineer may tune it.

Decided by the product owner (change if you disagree): the group takes the place of its topmost member; Ungroup one level at a time; maximum depth 32; groups are unnamed in this version ("Group"); the entered state is not saved; Escape order in criterion 17; a click selects the top-level group; empty groups are removed; the commit labels `group` and `ungroup`.

## UX notes

By `ux-engineer`, 2026-10-10. Numbers and rows are in `docs/design-system.md`: "Tool rail architecture", "Group toolbox", "Entered group", "Command glyphs", "Group selection box", "Rail tooltip and notice placement", "Canvas notice slot" (Interaction conventions), "Keyboard shortcuts established so far". The rail rules (toolbox cards, dimming, roving focus, commands that are not tools) are those of the Boolean toolbox and are not repeated here. Reference tools: Inkscape (groups behave as layers when entered, with a name in the status bar as the only sign), Illustrator (isolation mode: the rest dims, a bar names the path out), Affinity (double-click enters). Isolation mode is the model; the cues are the ones the criteria already ask for.

### Group and Ungroup: the Group card

- **Placement.** A card of two 40 px buttons, **Group** above **Ungroup**, the second card of column A, 8 px under the Tools card: 92 px high, ending 424 px below the viewport top. It replaces the plan "Group above Path in column B": column B is full (Path 4, Boolean 7), column A has 88 px free. If the Group release ships before the Pencil, the Tools card is 268 px and the Group card starts at 288 px; the Pencil release then moves it down 44 px, announced there. The Boolean card has to be in column B by this release (row "Tool rail architecture"; `0023` is one of the three releases that triggers the move). The "Where the commands live" section above is replaced by this.
- **States.** Group: dimmed with fewer than two objects selected or a tool other than Select. Ungroup: dimmed when the selection holds no group or the tool is not Select; with groups and other objects it is enabled and ungroups the groups only (the rest stays selected, criterion 3). Dimming is `aria-disabled="true"` at 40 % opacity, focusable, tooltip open on a press, activation does nothing.
- **Tooltips.** "Group (Ctrl+G)" / "Binds the selection into one group." / "Objects in between move below the group." (criterion 4: this is where the order change is shown); dimmed: "Select two or more objects." and, for another tool, "Use the Select tool.". "Ungroup (Ctrl+Shift+G)" / "Releases the objects of a group." / "One level only. Inner groups stay groups."; dimmed: "Select a group.". Cmd instead of Ctrl on macOS. These are the only rail tooltips with a chord, so the chord is written in the name line, not as a fourth line. "No undo yet." goes with `0020` criterion 52.
- **Glyphs.** Group: a rounded frame around two small solid squares. Ungroup: the two squares apart, the frame reduced to corner ticks. The pair must read apart at 1x on the glyph sheet. There is no Enter-group button: entering is a double-click or Enter, and the rail has no room for a command the spec gives no criterion.
- **Notices** go to the **canvas notice slot** (bottom centre) for the button and for Ctrl+G alike: "Grouped 3 objects." and "Ungrouped 2 groups into 7 objects." (3 s, `role="status"`); the depth refusal "Groups can be nested 32 levels deep. Nothing was changed." is an alert in the same slot, 8 s. A key and a button then produce one message in one place.
- **Refusals of other commands** on a selection with a group read "<Command> does not work on groups. Ungroup first. Nothing was changed." (criterion 28); the offending group is outlined in red (row "Refusal outline"), and the Select bar's Offset button is hidden for a group alone.

### Selecting, hovering, entering, leaving

- **Click** selects the group; the selection shows only the group box and its handles (`0019`), no member boxes. **Hover** over a group draws its box in `--hover-box`, which is what a click would select. **Double-click** on a group enters it. **Enter** with exactly one group selected and the canvas focused enters it with nothing selected. A hint chip (600 ms of rest over a selected group's box, away from handles) reads "Double-click to open" / "Enter opens it" and is `aria-hidden`; the keyboard route is in the table below and in the "Open groups" indicator once inside.
- **Entered state** (row "Entered group"): everything outside is drawn at 35 % strength, inert, with selection overlays at full strength; the indicator "Open groups" in the canvas notice slot shows `Document > Group > Group` as buttons, the last level as text, and a muted "Esc: leave". Dimmed black artwork is about 2.4:1 on the default canvas: it recedes and stays readable, and dimming is never the only cue.
- **Leaving** has three routes of different cost: the crumb "Document" (one press, every level), a double-click on empty canvas (one level), Escape (one level, after the selection is cleared). Escape order in the Select tool: an open chip or bar field; a drag; the selection; one level up, selecting the group just left; at the root nothing. From a selection two levels deep that is four presses, so the crumb is the fast way and the indicator says so.
- **Drawing inside** a group puts new objects on top of its children (criterion 19); the indicator makes the reason visible.
- A pan, zoom or tool change keeps the context and the dimming (criterion 21). A deleted or removed entered group moves the context to the nearest ancestor and the indicator updates in the same frame.

### Properties panel and Node tool

The subject line reads "Group of 5 objects" or "2 groups" (criterion 11, the texts of `0017`). The Style section edits every leaf; a differing value shows "Mixed" with the usual hatch swatch or empty field. The Node tool shows the nodes of every path leaf (criterion 12): for a group of several thousand paths this is as heavy as selecting them one by one, and the Node tool's node and handle limits for large selections apply unchanged.

### Keyboard, accessibility, contrast

- **Chords:** Ctrl+G group, Ctrl+Shift+G ungroup (Cmd on macOS), once per press, ignored on key repeat and when a drag, a chip or an unfinished Pen path is active (`0010` criterion 55); `preventDefault` only when they act, since a browser binds Ctrl+G to "find next". Enter and double-click enter; Escape and the crumbs leave. Tab reaches the Grouping toolbox as one stop (Tab order: Tools, Group, Path, Boolean), then the canvas, the bars, and the indicator's buttons after the bars.
- **Announcements** (hidden polite live region, not the notice slot): on entering "Inside group, 1 level. Escape leaves."; on leaving "Back at the document." or "Inside group, 1 level."; a selected group is announced by the selection announcer as "Group of 5 objects selected, 46.2 by 18.7 mm". Dimming alone tells a screen-reader user nothing, so these carry it.
- **Contrast and targets:** indicator text `--toolbar-icon` on `--toolbar-bg` 8.3:1, the muted hint 5.0:1; crumb buttons 28 px high; rail buttons 40 px; focus rings 2 px `--editor-accent`. The 35 % dimming is deliberately below 3:1 for objects that are not operable.

### At 800 x 600, large documents

The Group card is 92 px in column A (ends 424 px of 546); nothing else in the viewport moves. The indicator is at most 480 px wide and about 230 px at three levels, inside the 496 px viewport; beyond three levels the middle collapses. Dimming is one more draw pass at 35 % opacity for the outside content and costs about what a normal redraw costs; the group box over thousands of leaves is computed from cached outlines (`0019`), and member boxes are not drawn above 500 objects.

### Questions for the customer

1. **Dimming strength (Question 7).** 35 % as written. It is a constant in the design system (`--context-dim`) and one line to change after the demo.
2. **Shortcuts (Question 4).** Default in: Ctrl+G and Ctrl+Shift+G are what Inkscape, Illustrator and Affinity use. The cost is a `preventDefault` against the browser's "find next" in the browser build, only when the chord acts.
3. **Boolean card moves to column B** when the first of Group, Pencil, Fracture and Flatten ships (row "Tool rail architecture"). It is asked under `0032`; this release is one of the three that can trigger it.

## Links

Requirements: R-EDIT-009 (`docs/requirements.md`); related R-EDIT-012
Builds on: `specs/0019-multi-object-transform/` (the group box and its handles; must be built first), `specs/0014-advanced-selection/` (hit rules, Alt-click cycle, marquee), `specs/0010-edit-interaction-polish/` (Escape cascade, shortcut gating), `specs/0017-style-panel-rework/` (Style area, subject line, Mixed)
Amends: `specs/0016-boolean-operations/` (refusal for groups, criterion 28), `specs/0034-pen-path-extension/` (targets, criterion 30)
Related: `docs/adr/0002-document-model-units-and-svg-round-trip.md` §5 (tree, no inheritance), `specs/0039` (layers, placeholder in `specs/README.md`), `specs/0020` (undo, reserved)
ADRs: `adrs.md` (architect, to come; document model, accepted by the customer on 2026-10-10)
PR: TBD
