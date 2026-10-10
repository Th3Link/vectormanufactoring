# Split: separate the unconnected parts of a compound path, holes stay with their part

Status: Ready (2026-10-10: `adrs.md` and the UX notes exist; the customer's decision of 2026-10-10 settles what Split means; built on the default of Question 1: the built Break apart is renamed Split, and Break apart becomes the every-outline command, unless the customer vetoes)
Priority: Should
Origin: Customer (2026-10-10: "Split works like Inkscape's Split Path: it separates the parts that are not connected or overlapping; an outer outline with the holes inside it stays one object. Break apart separates every subpath, including holes. The operation that cuts paths where they cross is Cut."). The texts, numbers and tests are mine.

**Why a new spec and not an amendment of `0035`.** `0035-combine-and-break-apart` is Ready, has its `adrs.md` and UX notes, and is built as milestone M4 of the path-tools slice. The customer's decision changes what one of its two commands means and adds a third command to the same family. A table that sets Split, Break apart and Cut against each other belongs to none of the three, so it lives here; `0035` and `0036` point to it. `0035` gets a short "amendment pending" note.

## User value

As a maker I want to take a compound path apart into its separate pieces without losing the holes of a piece, so that a donut stays one donut and two circles that lie apart become two objects. Then I can move, restyle or delete one piece of a combined or subtracted shape, and every piece still cuts as the shape I drew.

**What the reference tools do.**

- Inkscape 1.2 added Split Path next to Break Apart. Its release notes show the word "Inkscape" as a path: Split Path gives 8 parts, one per letter, and Break Apart gives 12, because the holes in the letters become pieces of their own ([release notes 1.2](https://wiki.inkscape.org/wiki/Translations:Release_notes/1.2/325/en)). That is the rule we take: unconnected parts apart, holes with their part.
- Illustrator's Release Compound Path gives every subpath its own path, holes included, as far as I know. That is our Break apart.
- LightBurn has no equivalent for compound paths.

What we do better: three commands with plain names and tooltips that state the rule, an always visible dimmed state when a command does not apply, curves and nodes kept exactly, and a notice that says what happened and how many pieces came out.

## Fact check against `0035` (read this first)

`0035` as written (criterion 13) and as built keeps holes with their piece: a ring stays a ring, a compound path of two separate squares becomes two paths. **That is the customer's Split.** The customer describes Break apart as the command that separates every subpath, holes included, which `0035` lists as its Question 2, option B, "Release holes", and did not build. So the built command and the customer's wording do not match.

Default taken (Question 1, A): the built command is renamed **Split** (behaviour unchanged, only name, tooltip, commit label and glyph), and **Break apart** becomes the every-subpath command. Both are cheap: Split is the existing plan function; the new Break apart gives each outline its own object and needs no geometry. Option B in Question 1 keeps `0035` as it is and drops this spec.

## The three commands side by side

| | Split (this spec) | Break apart (`0035`, amended here) | Cut (`0036-cut-at-crossings`) |
|---|---|---|---|
| Works on | compound paths | compound paths | ordinary open or closed paths and primitives; compound paths are refused |
| Rule | one object per **part**: an outer outline with the holes inside it | one closed path per **outline**, holes included | the lines are cut open at every crossing, touch and self-crossing |
| Result | closed paths, and compound paths for parts with holes | closed paths only | open paths only |
| What the picture does | unchanged | a hole becomes a filled shape and covers its surroundings | unchanged lines, Fill set to None |
| Geometry | every node, handle and kind as before | every node, handle and kind as before | exact cuts on the curves, new end nodes |
| Button dimmed when | no compound path is selected | no compound path is selected | no object is selected |

### Worked example

Radii and offsets in millimetres. "Ring" means a compound path with an outline of radius 20 and a hole of radius 10, as Difference of two concentric circles makes it.

| # | Input | Split | Break apart | Cut |
|---|---|---|---|---|
| E1 | A ring | Refused, nothing changes: "Nothing to split. The compound path is one piece with its holes. Use Break apart to release the holes. Nothing was changed." | 2 closed paths: a disc of radius 20 and a disc of radius 10 on top of it. The ring now looks like a disc. | Refused: compound paths need Split or Break apart first |
| E2 | Two separate circles combined into one compound path | 2 closed paths | 2 closed paths (same as Split) | Refused: compound path |
| E3 | The same two circles as two ordinary objects | Dimmed: no compound path | Dimmed | Refused: "The selected paths do not cross each other or themselves." |
| E4 | Two overlapping circles (radius 10, centres 10 apart) as two objects | Dimmed | Dimmed | 4 open arcs, two per circle, meeting at the two crossing points (5, 8.66) and (5, -8.66) from the left centre |
| E5 | A ring with a disc of radius 5 in its hole, one compound path | 2 objects: the ring (compound path) and the disc of radius 5 (closed path) | 3 closed paths: radius 20, 10 and 5 | Refused: compound path |
| E6 | The Union of the two circles of E4 (one outline, ordinary closed path) | Dimmed | Dimmed | Refused: the outline does not cross or touch itself |

## Words used below

- **Operand:** a selected compound path.
- **Outline, depth, region:** as `0035` defines them. An outline is one closed contour. The depth of an outline is how many other outlines of the same compound path enclose it, tested with its first node by the nonzero rule. Even depth is a shape, odd depth is a hole. A **region** is one even-depth outline with the odd-depth outlines directly inside it.
- **Touch:** two outlines whose closest points are 0.001 mm or less apart, or that cross (the kernel grid of `0016` criterion 39).
- **Part:** a region, or several regions joined because an outline of one touches an outline of the other. A disc inside the hole of a ring is a region of its own and so a part of its own: it is not connected to the ring.

## Acceptance criteria

### Entry

1. Given any state of the app, then Split is a command in the Path card of the left rail (`0035` criterion 1a), directly after Combine (the card reads Combine, Split, Break apart, Cut; Split takes the place of the built second button, whose behaviour it keeps). Its button follows the rules of `0035` criterion 1: always rendered; dimmed (`aria-disabled`, still focusable, tooltip explains, activation does nothing) when the selection holds no compound path or a tool other than Select is active; never changes the tool; no shortcut. A pure function in `curvyo-ui-core` decides from the objects and the selection, and it is the same function that decides for Break apart (both are dimmed in the same cases). The frontend only shows it.
2. Given the tooltips (name, rule, note; the rail's rules), then Split reads "Split" / "Separates unconnected parts. Holes stay." / "Replaces the selection. No undo yet." and, amended, Break apart reads "Break apart" / "Every outline becomes its own object." / "Holes become filled shapes. Use Split to keep them. No undo yet." Each rule line is at most 45 characters. When Split or Break apart is dimmed the note says why, first match wins: no compound path "Select a compound path.", with another tool than Select adding "Use the Select tool." ("No undo yet." goes with `0020`, its criterion 52.)

### Split

3. Given a selection with at least one compound path that has more than one part, when the maker activates Split, then each such compound path is replaced, at its place in the stacking order, by one object per part, in the order of the first outline of each part. A part with holes is a compound path, a part without holes is an ordinary closed path. Test: E2 gives two closed paths; E5 gives the ring and the disc of radius 5.
4. Given two regions of one compound path of which an outline touches or crosses an outline of the other (a file written by another program can hold this, Combine refuses it), then they are one part and stay one object. Test: a compound path of two overlapping squares, loaded from a golden file, is one part: Split refuses it as in criterion 7. Two squares 0.01 mm apart are two parts.
5. Given criterion 3, then every node, handle and node kind of every piece is exactly as before (no flattening, no reversal), every piece has new object and anchor ids, and its complete style is a copy of the compound path's style.
6. Given criterion 3, then the pieces sit consecutively at the compound path's place, and all pieces are selected, together with the objects of the selection that were not split (objects that were not compound paths, compound paths of one part: all untouched, same ids). The active tool is unchanged.
7. Given a compound path of one part (a ring, a plate with holes, E1), then it is left as it is: same object, same ids, not rewritten. When no selected compound path has more than one part, Split is refused with "Nothing to split. The compound path is one piece with its holes. Use Break apart to release the holes. Nothing was changed." (plural: "Nothing to split. The 2 selected compound paths are one piece each, with their holes. Use Break apart to release the holes. Nothing was changed."), shown as a chip as `0035` criterion 16 shows its refusals (`role="alert"`, 8 seconds, no outline, nothing changed).
8. Given a successful Split, then exactly one commit is made, stored as `split`, atomic as `0016` criterion 28, and a notice says what happened, anchored and cleared as `0035` criterion 17: "Split: 1 compound path became 2 objects. No undo yet." (plural "2 compound paths became 5 objects."). When a piece has a hole, "Holes stayed with their piece." follows the first sentence; when compound paths of one part were left alone, "1 compound path is one piece and was left as it is." follows (plural "2 compound paths are one piece each and were left as they are."); "No undo yet." comes last.
9. Given a selection with no compound path (reached only through the session function, since the button is dimmed), then Split is refused with "Split needs a compound path. Nothing was changed." Given a group in the selection, then it is refused with "Split does not work on groups. Ungroup first. Nothing was changed." (`0023-groups` criterion 28). A refusal opens no dialog, keeps the buttons usable, and a second press gives the same result (`0016` criterion 18).
10. Given Combine (`0035`) and then Split on the same shapes, then every shape comes back as its own object, with the holes that enclosed it kept as holes, node for node as at the start (shapes inside shapes: a shape in a hole comes back as a separate object, as in E5). Given Split and then Combine, the result is again one compound path with the same outlines.

### Break apart (amended)

11. Given a selection with at least one compound path, when the maker activates Break apart, then each compound path is replaced, at its place, by one ordinary closed path per outline, in the compound path's outline order. Every node, handle and kind is as before, every piece has new ids, and its complete style is a copy of the compound path's style. Test: E1 gives two closed paths, E5 gives three, E2 gives two. A compound path of a single outline (only a hand-edited file holds one) becomes one closed path.
12. Given criterion 11, then all pieces are selected together with the objects that were not compound paths, the tool is unchanged, one commit is made, stored as `break_apart`, and the notice reads "Break apart: 1 compound path became 2 objects. No undo yet." (plural "2 compound paths became 5 objects."). When at least one piece was a hole (an odd-depth outline), "Holes became filled shapes." follows the first sentence. The look of the picture can change here, and the tooltip and this sentence are the warning. Break apart has no "one piece" refusal: every compound path with two or more outlines gives two or more pieces.
13. Given a selection with no compound path or with a group, then Break apart is refused as in criterion 9 with the name "Break apart".

### The three commands together

14. Given the six inputs E1 to E6 above, built by a deterministic helper inside the test, then Split, Break apart and Cut give exactly the results of the table (the enabled or dimmed state of the buttons, the refusal sentence, or the object counts and kinds), and the pieces of E4 under Cut end within 0.01 mm of the two crossing points.

### Performance and determinism

15. Given four inputs generated by a deterministic generator inside the test (not stored as fixture files): (a) one rectangle with 1000 circles of 4 nodes each inside, (b) 2000 disjoint squares in one compound path, (c) 500 nested squares in one compound path, each inside the previous with a gap, (d) a compound path of 5000 tiny squares, then Split and Break apart each complete in under 2 s in a release build on the reference desktop, without panic or hang, and give the golden counts. Split: (a) refused as one piece, (b) 2000 closed paths, (c) 250 compound paths with one hole each, (d) 5000 closed paths. Break apart: (a) 1001 closed paths, (b) 2000, (c) 500, (d) 5000. On input (a) the part search alone completes in under 500 ms.
16. Given the same objects at the same stacking positions, then the result is the same however they were selected and on every platform to 1e-9 mm: no arithmetic happens beyond ordering and copying nodes.

## Out of scope

- **Releasing one hole** and keeping the rest of the compound path together (a "Release holes" command). Break apart releases all of them.
- **Splitting an ordinary path** that crosses itself, or two separate paths that overlap: that is Cut (`0036-cut-at-crossings`) and Fracture (`0037-fracture-and-flatten`).
- **Open paths, text, images.** A compound path holds closed outlines only (`0016`).
- **Groups** (criterion 9) and splitting through them.
- **A shortcut** for any of the three commands, as for the Boolean operations (`0016` Question 3).
- **A preview of the result on hover.**
- **Undo and redo** (`0020`).

## Open questions

Each has a default; nothing blocks.

1. **Break apart and `0035`.** *A (default):* as the Fact check says: the built command becomes Split; Break apart becomes the every-subpath command (criteria 11 to 13), which is `0035` Question 2 option B promoted to the main command. This changes behaviour the customer has already seen in `0035`. *B:* `0035` stays as it is; there is no Split command and this spec is dropped, since Break apart already does what the customer describes for Split. Recommendation: A, because it matches the customer's wording of 2026-10-10 and Inkscape, where both commands exist.
2. **Touching parts stay together (criterion 4).** *A (default):* regions whose outlines touch or cross are one part. *B:* only nesting decides, as `0035` does today (two overlapping squares in one compound path would become two objects). Recommendation: A; it is what "not connected or overlapping" says. Only files from other programs can contain such a path.
3. **Names.** Default: "Split" and "Break apart" (customer). Inkscape calls the first "Split Path".

Decided by the product owner (change if you disagree): commit labels `split` and `break_apart`; pieces in the order of their first outline; no shortcuts; the performance inputs of criterion 15; Break apart has no "one piece" refusal.

## UX notes

By `ux-engineer`, 2026-10-10. Numbers and rows are in `docs/design-system.md`: "Tool rail architecture", "Path toolbox", "Path tooltips and notices", "Split, Break apart, Cut", "Command glyphs", "Action notice", "Refusal outline". The rail rules (toolbox cards, dimming, roving focus, tooltip and notice placement, commands that are not tools) are those of the Boolean toolbox and are not repeated here. Reference: `docs/reference-tools.md`, Inkscape (Split Path, Break Apart and Cut Path in one menu; the names do not say which keeps the holes).

### Telling the three apart

A maker meets Combine, Split, Break apart and Cut side by side in one 180 px card. What lets him choose without trying all three:

- **The selection enables the right ones.** A compound path enables Split and Break apart. An ordinary path or primitive enables only Cut; Split and Break apart stand dimmed and say "Select a compound path.". Two circles that are two objects therefore never offer Split by mistake.
- **The rule line names the result**, in the first words: Split "Separates unconnected parts. Holes stay." / Break apart "Every outline becomes its own object." / Cut "Cuts paths open where they cross or touch." Each is under 45 characters and never wraps.
- **The command that changes the look carries the warning in its note**: Break apart "Holes become filled shapes. Use Split to keep them." and Cut "Pieces get no fill." Split's note is only "Replaces the selection.", because Split never changes the picture.
- **Every refusal names the command to use instead** (Split on a one-piece compound path: "Use Break apart to release the holes."; Cut on a compound path: "Split or Break apart first."). A wrong guess costs one press and teaches the rule.
- **The glyphs show the result**, and the notices count what came out: "objects" for Split and Break apart, "pieces" for Cut.
- Inputs E1 to E6 above are the acceptance sheet for this: for each, the enabled or dimmed buttons and the sentence in the table are what the maker sees.

### Placement and order

All three are buttons of the Path card (column B, top). **Order: Combine, Split, Break apart, Cut.** Combine and Split are inverses (criterion 10) and sit together. Split is the safe one (the picture does not change) and takes the place of the built second button, whose behaviour it keeps, so nobody who used the built command finds it moved or changed. Break apart, the blunt one, comes after it. The criterion 1 wording "directly after Break apart" is changed to "directly after Combine" for this reason. The built command, glyph, notice text and commit label of Break apart become Split; Break apart is new work.

### States (one function decides for Split and Break apart, criterion 1)

| Situation | Split | Break apart |
|---|---|---|
| Select tool, no compound path in the selection | dimmed, "Select a compound path." | dimmed, the same note |
| Another tool than Select | dimmed, adds "Use the Select tool." | the same |
| A compound path in the selection | enabled | enabled |
| A group in the selection | enabled when a compound path is selected too; pressing refuses with the group sentence | the same |
| A ring (one part) | enabled; pressing refuses ("Nothing to split. ... Use Break apart to release the holes.") | enabled; pressing works |

Dimming is `aria-disabled="true"` at 40 % opacity, focusable, tooltip open, activation does nothing (the rail's rule). "Nothing to split" cannot be known without the part search, so it is a refusal on press and not a dimmed state.

### Texts

Tooltips, success notices and refusals are in the criteria and in the row "Path tooltips and notices". Changes made to the spec's wording (no scope change): the Break apart note now points to Split; the refusal of criterion 7 ends with "Nothing was changed." like every other refusal and points to Break apart; "When Split is dimmed" reads "When Split or Break apart is dimmed" (the same function decides for both). Success notices are 3 s, 5 s above 70 characters (the long Split notice), `role="status"`; refusals are `role="alert"`, 8 s, no outline for "one piece", the group sentence outlines nothing either (the whole selection is the offender). "No undo yet." in the quoted texts goes away with `0020` criterion 52.

### Glyphs

Split is the art of the built Break apart (a plate with its hole and a separate piece). Break apart is the same two pieces without the hole. The hole (3 units, about 3.75 px at the rail size) is the only difference, so the sheet decides: if the two do not read apart at 1x, Break apart becomes three solid squares stepping down the diagonal. Gaps between pieces are measured between stroke edges (row "Command glyphs"): the built art has 0.5 px visible and is redrawn.

### Keyboard, accessibility, contrast

No shortcut for any of the three (as the Boolean operations; Ctrl+X stays the clipboard's). Tab reaches the Path toolbox as one stop, Up and Down move, Space or Enter presses; after a key press the focus stays on the button, after a mouse press it returns to the canvas. Names are the plain words "Split" and "Break apart", also the accessible names. Dimmed glyphs at 40 % stay recognisable and the tooltip carries the reason, so state is never colour alone. Notice text `--toolbar-icon` on `--toolbar-bg` 8.3:1; refusal text `--field-invalid` 6.5:1 on its `--popover` ground.

### At 800 x 600, large documents

The Path card is 180 px of the 546 px height of column B (the card grows by 44 px per button; nothing else moves). A compound path of 5,000 pieces (criterion 15) is two commands under 2 s: the rail shows `aria-busy` and the wait cursor while it runs, as for the Boolean operations. After Split or Break apart all pieces are selected, and the member boxes are not drawn above 500 pieces (row "Member box"), so a big result shows the group box only.

### Questions for the customer

1. **The Node bar also has a "Split"** (break a path at a node, `0006`). It is a different object and appears only with the Node tool, where the rail dims, so the homonym is accepted. *Default:* keep both names. *Option:* rename the Node bar's button "Break at node".
2. Question 1 of this spec (Break apart as the every-outline command): the interaction design works with A, the default, and is consistent with Inkscape's pair. Nothing to add.

## Links

Requirements: R-EDIT-023 (`docs/requirements.md`)
Amends: `specs/0035-combine-and-break-apart/` (the command named Break apart there becomes Split; criteria 13 to 18 and Question 2 change, once Question 1 is confirmed)
Related: `specs/0036-cut-at-crossings/`, `specs/0037-fracture-and-flatten/`, `specs/0016-boolean-operations/` (compound path, winding, refusals), `specs/0023-groups/` (groups refused)
ADRs: `adrs.md` (architect, to come; small: the part search reuses the region plan of `0035`)
PR: TBD
