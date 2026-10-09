# Combine and Break apart: several closed shapes into one compound path, and back

Status: Ready (`adrs.md` and the UX notes exist, both 2026-10-10)
Priority: Should
Origin: Customer (Combine, and Break apart "into separate paths: what happens to the holes", request of 2026-10-09, with the wish that the UX be much better than Inkscape's). The hole rule, the refusals, the names and the defaults are my proposals, marked in "Decided by the product owner".

## User value

As a maker I want to select the outline of a plate and the circles of its mounting holes and make them **one object with holes**, and to take such an object apart again without losing its holes, so that the laser cuts one clean job, the holes are real holes, and I never have to run a Boolean difference just to get a plate with holes.

**What Combine and Break apart are, and are not.** Combine puts the outlines of several closed shapes into **one compound path** (the object type `0016-boolean-operations` introduced), unchanged: nothing is flattened, nothing is moved. A Boolean operation computes new geometry; Combine only changes which object the outlines belong to, and sets the winding so that a shape inside another becomes a hole. Break apart is the reverse.

**Where Inkscape's equivalents are weak, and what we do instead** (as far as I know its behaviour; check against the customer's version):

- *Hidden and cryptically named.* Combine (Ctrl+K), Break Apart (Shift+Ctrl+K), and a newer Split Path sit in the Path menu among Union, Difference, Exclusion, Division and Cut Path. Here: buttons with names in plain words and a tooltip that states the rule.
- *Surprising holes.* Inkscape's Break Apart turns the hole of a ring into a separate filled shape, which then covers the ring, so a ring becomes a disc; a second, similarly named command is needed to keep holes. And a combined path with crossing outlines is painted even-odd, so a crossing silently makes a hole. Here: **Break apart keeps every hole with its shape**, and **Combine refuses outlines that touch or cross** and says which, instead of guessing.
- *No feedback.* Nothing says what happened. Here a one-line notice names the result and the counts.
- *Curves kept.* Unlike our Boolean operations, which return straight segments (`0016`, "What is worse than Inkscape"), Combine and Break apart keep every node, handle and node kind exactly. A combined pair of circles has the same eight nodes as before.

## Words used below

- **Operand:** a selected object that takes part in Combine: a closed ordinary path, a compound path, a rectangle, an ellipse, a polygon or a star. A primitive contributes its outline as "Object to path" would make it, with its curves.
- **Outline:** one closed contour of an operand (a compound path has several).
- **Depth** of an outline: how many other outlines of the operands (all together) enclose it. Depth 0 is outermost. An outline of **even depth** is a **shape**, an outline of **odd depth** is a **hole**. Enclosure is tested with a point of the outline (its first node) against the other outlines by the nonzero rule, as the canvas paints.
- **Region:** one even-depth outline together with the odd-depth outlines directly inside it.
- **Winding rule:** as fixed by `0016` criterion 41 (documented in its ADR): shapes run one way, holes the other.
- **Touch:** two outlines whose closest points are 0.001 mm or less apart (the kernel grid, `0016` criterion 39), or that cross.

## Acceptance criteria

### Entry points

1. Given any state of the app, then Combine and Break apart are two commands, always rendered, whose buttons follow the rules of `0016` criteria 1, 1a, 2 and 3 for the Boolean buttons: dimmed with `aria-disabled` when they do not apply, still focusable, with a tooltip of three lines (name, rule, note); activating a dimmed button does nothing; the active tool never changes; no shortcut. Combine is dimmed when fewer than two objects are selected or a tool other than Select is active. **Break apart is dimmed when the selection holds no compound path** (nothing selected, only other objects, or a tool other than Select active), and enabled when it holds one or more; an open path in the selection leaves Combine enabled (pressing refuses and outlines it, criterion 9). The interface can know "no compound path" without running anything, so that case is dimmed and not an error; Combine with touching shapes needs the kernel and is refused on press. A pure function in `curvyo-ui-core`, as `boolean_availability` is, decides from the objects and the selection; the frontend only shows it. The two buttons are the **Path card**, the first card of a second rail column (see 1a).
1a. Given the rail, then column B is drawn from this release: left edge 68 px (12 + 48 + 8), top level with column A (12 px inset), holding the Path card (48 px wide, 92 px high, two 40 x 40 px buttons, Combine above Break apart, 4 px apart, `role="toolbar"`, name "Path operations", one Tab stop; Tab order: Tools, Boolean, Path, canvas, bars). `--rail-right` is 116 px (was 60 px) and everything keyed to it moves by 56 px: the overlay row of the bars (`left` 72 px to 128 px), the default view on New and Open (document top-left 72 px to 128 px), the Boolean and Path notices (left edge `--rail-right` + 12), and the tooltips of all rail buttons (they open 6 px right of the 116 px edge, so they never cover the Path card). Test, Tauri window 800 x 600 with the panel open and rulers on: the whole rail (both columns) is visible and no button is clipped; the bars' row is 356 px wide (612 px with the panel collapsed); the Node bar and the polygon/star bar each fit on one row. Column A, its cards and gaps, the panel, the rulers and the status bar do not move.
2. Given the tooltips (name, rule, note; 400 ms, right of `--rail-right`, also on keyboard focus), then Combine reads "Combine" / "One object. Inner shapes become holes." / "Replaces the selection. No undo yet." and Break apart reads "Break apart" / "Splits a compound path into its pieces." / "Holes stay with their piece. Replaces the selection. No undo yet." Every rule line is at most 45 characters, so it never wraps. When the button is dimmed or Combine is blocked the note says why, first match wins: Combine: fewer than two selected "Select two or more closed shapes." (another tool than Select adds "Use the Select tool."), an open path "Needs closed paths: 1 of 3 selected is open."; Break apart: no compound path selected "Select a compound path." (another tool than Select adds "Use the Select tool."). The glyphs: Combine shows one solid square with a square hole (one object with a hole); Break apart shows a smaller plate with its hole and, apart from it, a separate solid square: the hole stays in the plate, it is not released as its own shape. The two read apart at 1x, and apart from Union and Offset.

### Combine

3. Given two or more operands whose outlines do not touch each other (no two outlines of the operands, of different or of the same operand, touch), when the maker activates Combine, then all operands are removed and one new object takes their place: a compound path that holds every outline of every operand. Outline order in the result is the stacking order of the operands from the bottom, and within an operand its own outline order.
4. Given criterion 3, then the winding of every outline is set by its depth: even depth runs as the shapes of the winding rule, odd depth the opposite way. Reversing an outline reverses its node order and swaps every node's incoming and outgoing handle, so the drawn curve is unchanged. Test: two concentric discs of radius 20 mm and 10 mm, drawn in either direction and either stacking order, combine into a ring: the canvas paints a pixel at radius 15 mm and does not paint the centre; the painted area is π(400 − 100) ≈ 942.5 mm² within 1 mm² (the Bézier circle is not an exact circle). A third disc of radius 5 mm inside the hole is painted again (depth 2).
5. Given criterion 3, then nothing is flattened: the result has exactly the nodes of the operands (a primitive as many as Object to path gives it), each with its position, handles and kind unchanged except for the reversal of criterion 4. Test: two circles of four nodes each side by side give a compound path of eight nodes, all with their original positions and kinds.
6. Given the result, then its complete style is a copy of the **bottom-most** operand's style, and it sits at the bottom-most operand's place in the stacking order (`0016` criteria 21 and 22 with "base operand" = bottom-most). Test: a large red rectangle under three small blue circles gives a red plate with three holes.
7. Given a successful Combine, then the new object is the only selected object, the active tool is unchanged, the Properties panel describes it ("Compound path"), the result's rotation is 0, and the document records exactly one commit, stored as `combine_paths`, atomic as `0016` criterion 28. A notice names the result and the counts and ends with "No undo yet.", as `0016` criterion 29: "Combine: 4 objects became 1 compound path with 3 holes. No undo yet." The holes are the outlines of odd depth; "with N holes" is omitted when N is zero and reads "with 1 hole" for one. When the complete styles of the operands are not all equal, one more sentence comes before "No undo yet.": "It uses the style of the lowest object." The notice is shown in the status region, anchored with its left edge at `--rail-right` + 12 px and its top level with the Combine button, and lasts 3 seconds, or 5 seconds when the sentence is longer than 70 characters. A press anywhere, a key, a selection change or a tool change clears it.
8. Given the same objects at the same stacking positions, then the result is the same however they were selected (`0016` criterion 9) and on every platform to 1e-9 mm, since no arithmetic happens beyond reversing node order.

### Combine: refusals

"Nothing changes" means what it means in `0016`: the same objects with the same ids, order, geometry and styles, the same selection and tool, no commit.

9. Given an operand that is an open path, then Combine is refused with "Combine needs closed paths. 1 of 3 selected objects is open. Nothing was changed.", shown as `0016` criterion 15 shows it (a chip beside the rail for 8 seconds, each offending operand outlined in red with white casing, `role="alert"`). (Question 1.)
10. Given outlines that touch or cross, then Combine is refused with "Combine needs shapes that do not touch. 2 of 3 selected objects touch each other. Use Union to merge overlapping shapes. Nothing was changed.", the objects involved outlined in red as above. Test: two overlapping squares are refused; two squares 0.01 mm apart combine; two squares sharing an edge are refused. Outlines that touch or cross are refused as a pair of different outlines (offenders: every object that touches another).
10a. Given an operand whose own outline crosses or touches itself (a figure eight), or a compound path whose own outlines touch each other, and no pair of different objects that touch, then Combine is refused with a sentence of its own: "Combine needs shapes that do not cross themselves. 1 of 3 selected objects does. Nothing was changed.", with that object outlined in red. It is the same check and the same refusal as for touching shapes (a self-crossing outline has no meaningful winding by depth), only the sentence differs. When both cases occur, the touching sentence is shown first; a second press after the maker fixes it shows the other. Test: a figure-eight path with two separate squares is refused with the self-crossing sentence and only the figure eight outlined.
11. Given an operand that is a group, then Combine is refused with "Combine does not work on groups. Ungroup first. Nothing was changed." (`0023-groups` criterion 28.) Given an operand that encloses no area (all nodes on one line), then it is refused as `0016` criterion 16 says. Given a coordinate that is not finite or beyond `MAX_COORDINATE_MM` (`0016` criterion 17a), then it is refused as there.
12. Given a refusal, then no modal dialog opens, the buttons stay usable, and a second activation gives the same result (`0016` criterion 18).

### Break apart

13. Given a selection that contains at least one compound path, when the maker activates Break apart, then each compound path is replaced, at its place in the stacking order, by one object per **region** (definition above): an even-depth outline with the outlines directly inside it as holes. A region with holes is a compound path, a region without holes is an ordinary closed path (the Node tool edits it like any other). A compound path that has only one region (a ring, a plate with holes, no separate shape) is already one piece with its holes: it is **left as it is**, the same object with the same ids, not rewritten and not counted as broken apart, and criterion 15 and 17 report it. Test: a ring (outline 20 mm, hole 10 mm) with an island (5 mm) inside the hole gives two objects: the ring (a compound path with a hole) and the disc of 5 mm (a closed path). A compound path of two separate squares gives two closed paths.
14. Given criterion 13, then every node, handle and kind is exactly as before (no flattening, no reversal: the windings were already in the canonical form), every piece has new object and anchor ids, and each piece's complete style is a copy of the compound path's style.
15. Given criterion 13, then the pieces sit consecutively at the compound path's place in the stacking order, in the order of the first outline of each region, and all pieces are selected, together with the objects of the selection that were not broken apart: objects that were not compound paths and compound paths with one region, all untouched. The active tool is unchanged.
16. Given Break apart is refused, then it is by one of two refusals, each shown as a chip as in criterion 9 (`role="alert"`, 8 seconds, no outline, nothing changed):
    a. **No compound path in the selection** (defensive: the button is dimmed in this case, criterion 1, so it is reached only through the session function, not by a press): "Break apart needs a compound path. Nothing was changed."
    b. **No selected compound path has more than one region:** "Nothing to break apart. The compound path is one piece with its holes." (plural: "Nothing to break apart. The 2 selected compound paths are one piece each, with their holes."). Test: a ring alone is selected, Break apart is pressed: the refusal shows, no commit is made, the object keeps its ids and the selection is as before.
17. Given a successful Break apart (at least one selected compound path with more than one region), then one commit is made, stored as `break_apart`, atomic, and a notice names the result and the counts, anchored and cleared as in criterion 7 (left edge `--rail-right` + 12 px, top level with the Combine button, which is the first button of the Path card; 3 seconds, or 5 seconds when longer than 70 characters). The sentence is "Break apart: 1 compound path became 3 objects. No undo yet." (plural "2 compound paths became 5 objects."); when a piece has a hole, "Holes stayed with their piece." follows the first sentence; when compound paths of one region were left alone, "1 compound path is one piece and was left as it is." follows (plural "2 compound paths are one piece each and were left as they are."); "No undo yet." comes last.
18. Given Combine then Break apart on the same shapes, then every shape comes back with the holes that enclosed it as holes (criterion 13), as ordinary closed paths or compound paths; the geometry is node for node as at the start. Given Break apart then Combine, the result is again one compound path with the same outlines.

### Performance and robustness

19. Given four inputs generated by a deterministic generator inside the test (not stored as fixture files): (a) one rectangle with 1000 circles of 4 nodes each inside, (b) 2000 disjoint squares, (c) 500 nested squares each inside the previous, (d) a compound path of 5000 tiny squares (as `0016` criterion 40 (i)), then Combine and Break apart each complete in under 2 s, in a release build on the reference desktop, without panic or hang, and give the expected counts, which are the golden values of the test: (a) one compound path of 1001 outlines, (b) one of 2000, (c) one of 500 with alternating windings, (d) 5000 closed paths. On input (a) the geometry part alone (the touch test and the nesting) completes in under 500 ms. The round trip of criterion 18 has a real golden file, `combine_ring_island.curvyo` (the ring with an island of criterion 13, saved after Combine), which opens and breaks apart into the expected two objects.

## Out of scope

- **Combining open paths** into one object. A compound path holds closed outlines only (`0016`); open subpaths are a model change (Question 1).
- **Crossing or overlapping outlines.** Use Union, Exclusion or Fracture (`0037`).
- **Releasing the holes** of a compound path as their own objects (Question 2).
- **Splitting a path at a node** (`0006`, the Node tool) and **at crossings** (`0036-split-at-crossings`).
- **Combine and Break apart through groups** (`0023-groups` criterion 28).
- **Changing the fill rule.** The fill is nonzero and the winding encodes the holes.
- **A shortcut** for either command, as for the Boolean operations (`0016` Question 3).
- **Undo and redo** (`0020`).
- **A preview of the result on hover**, and **marking where outlines touch** with a ring at each contact (the two offending objects outlined are enough; a later polish if makers ask).

## Open questions

Each has a default; nothing blocks.

1. **Open paths in Combine.** *A (default):* refused, closed only (`0016` criterion 15). *B:* allowed, which needs compound paths to hold open outlines (a stored open flag per outline, `format_version` bump). Useful for a plotter or a cut-order job where several lines should be one object. Recommendation: A now, B when a job story asks.
2. **Release holes.** *A (default):* Break apart keeps holes with their piece; there is no way to turn a hole into a free shape. *B:* a second command "Release holes" that gives every outline its own object (Inkscape's Break Apart). Recommendation: A; B is easy to add later because the pieces are the same outlines.
3. **Crossing outlines.** *A (default):* refused. *B:* combine anyway and let the nonzero fill decide (same-winding crossings paint as a union, opposite as holes), which is what Inkscape does with even-odd fill. Recommendation: A, because the result of B depends on drawing direction.
4. **Names.** Default: "Combine" and "Break apart". Option: "Merge into one object" and "Separate pieces".

Decided by the product owner (change if you disagree): depth by nesting decides shape and hole; the base operand is the bottom-most; the pieces keep the compound path's style; commit labels `combine_paths` and `break_apart`; no shortcuts; the performance fixtures of criterion 19.

## UX notes

By `ux-engineer`, 2026-10-10. Numbers and rows are in `docs/design-system.md`: "Tool rail architecture", "Path toolbox", "Command glyphs", "Path tooltips", "Action notice", "Refusal outline". The rail rules (toolbox cards, dimming, roving focus, tooltip and notice placement, commands that are not tools) are those of the Boolean toolbox and are not repeated here. Reference tools: Inkscape hides Combine and Break apart in the Path menu under names that do not say what they do, Break apart there turns the hole of a ring into a filled shape, and nothing reports a result. Here: two buttons that are always visible, a tooltip that states the rule and the effect on the selection, a notice with counts, and a refusal that names the offending objects.

### Names

**Combine** and **Break apart**, as in the spec (Question 4, default). They are the terms makers already know from Inkscape, Illustrator and Affinity, and the buttons are icon-only, so the tooltip and the notice carry the meaning in plain words: "One object. Inner shapes become holes." and "Splits a compound path into its pieces. Holes stay with their piece." The alternative "Merge into one object" is rejected because "merge" already means three other things here (Union, Join of two nodes, `0034` connect). The word "compound path" stays in the notices and the panel subject line because it is the object's name in the panel; it is explained once, in the Combine notice, by "with 3 holes".

### Placement: the Path card, and column B ships with this slice

This is the first release with a second rail column.

- **Column B** is drawn from this release: left edge 12 + 48 + 8 = **68 px**, top level with column A (12 px inset). It holds one card, **Path**: 48 px wide, 92 px high (4 + 40 + 4 + 40 + 4), two 40 x 40 px buttons, Combine above Break apart, 4 px apart, 20 px glyphs, the look of the Boolean card. It ends 104 px below the viewport top, just below the Pen button. No heading, `role="toolbar"`, `aria-orientation="vertical"`, name "Path operations", one Tab stop. Order of the rail in the Tab sequence: Tools (six stops), Boolean, Path, canvas, bars.
- **`--rail-right`** goes from 60 px to **116 px**. Everything keyed to it moves by 56 px, and nothing else changes: the overlay row of the bars (`left`) 72 px to **128 px**; the default view on New and Open (document top-left) 72 px to 128 px; the Boolean notice and the new Path notices (left edge `--rail-right` + 12) 72 px to 128 px; the tooltips of the Tools and Boolean buttons open 6 px right of the **116 px** edge, not of the 60 px edge, so they never cover the Path card (they sit about 66 px from their own button; the pressed button's hover ground ties them back, as the rail rules say). Tooltips of the Path buttons open at 122 px as well.
- **At 800 x 600** (Tauri, panel open, rulers on; viewport about 496 x 546): column A unchanged (ends 512 px, 34 px clear), column B 12 to 104 px. The bars' row shrinks from 412 px to **356 px** (496 - 128 - 12); with the panel collapsed it is 612 px. The Node bar (about 300 px) and the polygon/star bar (about 310 px) still fit on one row. The Select bar wraps by whole groups as before and gets taller; the implementer measures every Select bar state at build and reports the row counts. If the settings group (the two switches) alone is wider than 356 px, its two switches wrap inside the group, the first row keeps its y, and that is reported to the lead. A 360 px notice at 128 px needs 488 px of the 496: it fits; the notice clamps to the viewport as always.
- **What does not move:** column A, its cards, its gap, the Boolean notice's top (level with Union), the panel, the rulers, the status bar. Nothing is reflowed by window size. The Path card never scrolls (92 px); the "too short" rule of the rail applies to column A only.
- **Later growth:** `0023` adds the Group card above Path (Path moves down 144 px in that release, announced there), `0036` to `0038` add buttons below Combine and Break apart in the Path card. The card order inside it is fixed by `docs/design-system.md`.
- **Why the Path card is the top of column B and not a second row of column A:** column A has 500 px of 546 px used. A third card there would not fit at 800 x 600, and the maker would see commands that need the Select tool far from it. Column B puts Combine and Break apart right of the Select and Pen buttons, which is where the hand already is when a selection has just been made.

### Buttons, glyphs, states

Glyphs: 16 x 16 viewBox drawn at 20 px, 1.5 px stroke rules of the row "Command glyphs". **Combine:** one solid rounded square (11 x 11, 1 px radius) with a concentric square hole (5 x 5, evenodd): one object with a hole. **Break apart** (brief corrected): a smaller solid square with a hole (8 x 8, hole 3 x 3) at the upper left and, separated from it by a 1.6 px gap, a solid 4 x 4 square at the lower right: a plate with its hole stays one piece, and a separate piece leaves. The earlier brief ("the outline and the hole as two pieces") showed Inkscape's behaviour, the hole released, which is the opposite of the rule here. Combine and Break apart must read apart at 1x (one ring against a ring plus a piece), and both apart from Union (overlap) and Offset (two hollows); approved on a glyph sheet at 1x, 1.25x and 2x.

| Situation | Combine | Break apart |
|---|---|---|
| Select tool, nothing or one object selected | dimmed | dimmed if no compound path is selected; enabled with one |
| Select tool, two or more objects, none a compound path | enabled | dimmed |
| Select tool, a compound path in the selection | enabled with two or more objects | enabled |
| Any tool other than Select | dimmed | dimmed |
| An open path in the selection | enabled, tooltip note says how many are open; pressing refuses and outlines them | unchanged |

Dimmed is the rail's: `aria-disabled="true"`, 40% opacity, still focusable, no hover ground, default cursor, activation does nothing, the tooltip explains. Dimming means "not available now" and nothing else: Break apart is dimmed when the selection holds no compound path because the interface can know that without running anything, and an error for it would be noise. Combine with touching shapes is not known without the kernel, so it is enabled and refused on press.

**Tooltips** (the rail's: 400 ms, right of `--rail-right`, 260 px, name semibold, rule, muted note; also on keyboard focus). Rule lines are kept under 45 characters so they never wrap:

| Button | Name | Rule | Note, first match wins |
|---|---|---|---|
| Combine | Combine | One object. Inner shapes become holes. | fewer than two selected: "Select two or more closed shapes."; another tool than Select: add "Use the Select tool."; an open path: "Needs closed paths: 1 of 3 selected is open."; else "Replaces the selection. No undo yet." |
| Break apart | Break apart | Splits a compound path into its pieces. | no compound path selected: "Select a compound path." (with another tool "Use the Select tool."); else "Holes stay with their piece. Replaces the selection. No undo yet." (the note may wrap) |

### Feedback

All notices use the "Action notice" row: text only, no focus, one status region and one alert region for the whole rail (in the tree from the start), anchored **left edge `--rail-right` + 12, top level with the first button of the card that issued it**: a Combine or Break apart notice sits level with Combine (12 px below the viewport top), a Union notice level with Union. A press anywhere, a key, a selection change or a tool change clears it. Success lasts 3 s, or 5 s when the sentence is longer than 70 characters. Refusal: `role="alert"`, 8 s.

Success:
- "Combine: 4 objects became 1 compound path with 3 holes. No undo yet." ("with N holes" when N is above zero: holes are the outlines of odd depth; "with 1 hole").
- When the operands did not all have the same style, one more sentence before "No undo yet.": "It uses the style of the lowest object." Without it a blue circle that became a hole simply vanishes, and the plate keeps its red with no explanation.
- "Break apart: 1 compound path became 3 objects. No undo yet." With holes in the pieces: "Break apart: 1 compound path became 3 objects. Holes stayed with their piece. No undo yet." With a compound path that was one piece and was left alone: "... 1 compound path is one piece and was left as it is. ..." Plural forms "2 compound paths became 5 objects."

Refusals (all end "Nothing was changed."), with the offending objects drawn in the "Refusal outline" (hollow 2 px `--field-invalid`, white casing, not stored, not selected, not hit-testable) while the alert is up; at most 200 offenders are outlined, the count in the sentence is exact:
- open paths: "Combine needs closed paths. 1 of 3 selected objects is open." (outlined: the open paths)
- touching: "Combine needs shapes that do not touch. 2 of 3 selected objects touch each other. Use Union to merge overlapping shapes." (outlined: every object that touches another, both of a pair)
- a shape that crosses itself, alone: "Combine needs shapes that do not cross themselves. 1 of 3 selected objects does." (outlined: that object)
- no area, out of range, group: the sentences of `0016` with "Combine" for the operation, and "Combine does not work on groups. Ungroup first."
- Break apart, a compound path with one piece only: "Nothing to break apart. The compound path is one piece with its holes." (plural "The 2 selected compound paths are one piece each, with their holes."); no outline.

A second press repeats the notice. No dialog ever opens. A mouse press returns focus to the canvas, a key press leaves it on the button, after success and after refusal (the rail rule). While a call runs: `aria-busy` on the toolbar, cursor `wait`, other activations ignored (the Boolean rule; the `0035` budgets are under 2 s).

### What the maker sees after Break apart

Nothing moves, changes colour or animates: every piece has the compound path's geometry and style, in the same stacking place. What changes is the selection: **all pieces are selected** (criterion 15), so the canvas shows the group selection box around the whole and a dashed member box on each piece (the member boxes of `multi-object-transform`; not drawn above 500 pieces, then the box and the count in the notice are what there is), and the panel's subject line reads "3 paths", "2 compound paths" or "4 objects" by the mix. That, and the notice, is the whole feedback: it is the same view as after selecting the pieces by hand. Combine after it works at once (two or more selected), which makes the pair easy to reverse by hand. After Combine the one new compound path is selected: the panel says "Compound path", the holes show whatever lies behind the plate.

### Accessibility

Toolbar `aria-label` "Path operations"; buttons named by the bare names (no shortcut text, none exists); rule and note are the description (`aria-describedby`, also on focus); dimmed state announced by `aria-disabled`; glyph contrast 8.3:1 on `--toolbar-bg`; hit target 40 x 40 px; the cause of every refusal is text, with the red outline as the pairing; one live region each for status and alert so a screen reader hears the sentence once. Not colour alone: dimming is paired with the note, the outline with the sentence. No motion.

### Open design questions (defaults taken)

1. **Break apart dimmed without a compound path** (default) or enabled with a refusal (the spec's criterion 16 and the Boolean precedent for open paths). Dimmed is calmer and exact; criterion 16 then covers only the defensive path.
2. **One-piece compound path.** *Default:* left untouched (no new ids, no commit if all are such) and refused with "Nothing to break apart." *Alternative:* rewritten to an identical object and reported as "became 1 object", which is a commit that changes nothing.
3. **The style sentence on Combine** when styles differ. *Default:* yes.
4. **Show where outlines touch** (a small ring at each contact) on a touching refusal. *Default:* no, the two objects outlined are enough; a later polish if makers ask.
5. **Preview of the result on hover** (the Boolean preview, not built for Boolean either). *Default:* none for these two commands.
6. **The 56 px of canvas the second column takes** at 800 x 600. *Default:* accepted (customer decision of 2026-10-10: two columns). The Path card is only 92 px high, so the second column covers 48 x 92 px of canvas, not a strip.

### Criteria changes requested (by number)

Applied to the criteria above on 2026-10-10: 1 and new 1a (Break apart dimmed without a compound path; the Path card and `--rail-right` 116 px), 2 (rule lines under 45 characters, notes, glyph brief), 7 (hole count, style sentence, 5 s), new 10a (a shape that crosses itself gets its own sentence, same refusal as touching shapes; architect flag 1), 13, 15, 16, 17 (a one-region compound path is left as it is; 16 split into a defensive refusal and "Nothing to break apart"; notice text), 19 (inputs generated in the test; architect flag 2).

## Delivery

Milestone M4 of the path-tools slice (the four milestones of `adrs.md`), delivered in one PR together with the toolbox fix of `0016-boolean-operations`, `0031-segment-drag-bending` and `0034-pen-path-extension` (customer rule: one PR per slice). It follows 0034 because both touch `path_model.rs` (`reversed_anchors`) and the same crates. The second rail column (criterion 1a) ships with this milestone.

## Links

Requirements: R-EDIT-023 (`docs/requirements.md`); related R-EDIT-003
Builds on: `specs/0016-boolean-operations/` (compound path, winding rule, rail commands, refusal notices; must be merged first)
Related: `specs/0023-groups/` (refusal for groups), `specs/0036-split-at-crossings/`, `specs/0037-fracture-and-flatten/`, `specs/0006-path-merge-split-and-node-types/` (node-level Join and Split)
ADRs: `adrs.md` (architect, 2026-10-10)
PR: TBD (shared path-tools PR)
