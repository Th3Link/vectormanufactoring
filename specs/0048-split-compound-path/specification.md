# Split: separate the unconnected parts of a compound path, holes stay with their part

Status: Draft (criteria complete; the customer's decision of 2026-10-10 settles what Split means; one question about Break apart has a default, see Question 1; becomes Ready when `adrs.md` and the UX notes exist, `CLAUDE.md` §4)
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
| Button dimmed when | no compound path is selected | no compound path is selected | fewer than one object is selected |

### Worked example

Radii and offsets in millimetres. "Ring" means a compound path with an outline of radius 20 and a hole of radius 10, as Difference of two concentric circles makes it.

| # | Input | Split | Break apart | Cut |
|---|---|---|---|---|
| E1 | A ring | Refused, nothing changes: "Nothing to split. The compound path is one piece with its holes." | 2 closed paths: a disc of radius 20 and a disc of radius 10 on top of it. The ring now looks like a disc. | Refused: compound paths need Split or Break apart first |
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

1. Given any state of the app, then Split is a command in the Path card of the left rail (`0035` criterion 1a), directly after Break apart. Its button follows the rules of `0035` criterion 1: always rendered; dimmed (`aria-disabled`, still focusable, tooltip explains, activation does nothing) when the selection holds no compound path or a tool other than Select is active; never changes the tool; no shortcut. A pure function in `curvyo-ui-core` decides from the objects and the selection, and it is the same function that decides for Break apart (both are dimmed in the same cases). The frontend only shows it.
2. Given the tooltips (name, rule, note; the rail's rules), then Split reads "Split" / "Separates unconnected parts. Holes stay." / "Replaces the selection. No undo yet." and, amended, Break apart reads "Break apart" / "Every outline becomes its own object." / "Holes become filled shapes. Replaces the selection. No undo yet." Each rule line is at most 45 characters. When Split is dimmed the note says why, first match wins: no compound path "Select a compound path.", with another tool than Select adding "Use the Select tool." ("No undo yet." goes with `0020`, its criterion 52.)

### Split

3. Given a selection with at least one compound path that has more than one part, when the maker activates Split, then each such compound path is replaced, at its place in the stacking order, by one object per part, in the order of the first outline of each part. A part with holes is a compound path, a part without holes is an ordinary closed path. Test: E2 gives two closed paths; E5 gives the ring and the disc of radius 5.
4. Given two regions of one compound path of which an outline touches or crosses an outline of the other (a file written by another program can hold this, Combine refuses it), then they are one part and stay one object. Test: a compound path of two overlapping squares, loaded from a golden file, is one part: Split refuses it as in criterion 7. Two squares 0.01 mm apart are two parts.
5. Given criterion 3, then every node, handle and node kind of every piece is exactly as before (no flattening, no reversal), every piece has new object and anchor ids, and its complete style is a copy of the compound path's style.
6. Given criterion 3, then the pieces sit consecutively at the compound path's place, and all pieces are selected, together with the objects of the selection that were not split (objects that were not compound paths, compound paths of one part: all untouched, same ids). The active tool is unchanged.
7. Given a compound path of one part (a ring, a plate with holes, E1), then it is left as it is: same object, same ids, not rewritten. When no selected compound path has more than one part, Split is refused with "Nothing to split. The compound path is one piece with its holes." (plural: "Nothing to split. The 2 selected compound paths are one piece each, with their holes."), shown as a chip as `0035` criterion 16 shows its refusals (`role="alert"`, 8 seconds, no outline, nothing changed).
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

(filled in by ux-engineer before Ready)

For the ux-engineer: `docs/design-system.md` plans the Path card with six buttons (Combine, Break apart, Split at crossings, Fracture, Flatten, Offset). With the customer's decision it has seven, in this order: Combine, Break apart, Split, Cut, Fracture, Flatten, Offset; the row "Tool rail architecture" and its budget need the change, and "Split at crossings" in the rows "Path toolbox" and "Path tooltips" becomes "Cut". The glyph that `0035` gives Break apart (a plate with its hole and a separate piece) is now Split's glyph; Break apart gets one that shows the outline and its hole as two separate pieces. The three glyphs have to read apart at 1x. Names and notices are in the criteria above.

## Links

Requirements: R-EDIT-023 (`docs/requirements.md`)
Amends: `specs/0035-combine-and-break-apart/` (the command named Break apart there becomes Split; criteria 13 to 18 and Question 2 change, once Question 1 is confirmed)
Related: `specs/0036-cut-at-crossings/`, `specs/0037-fracture-and-flatten/`, `specs/0016-boolean-operations/` (compound path, winding, refusals), `specs/0023-groups/` (groups refused)
ADRs: `adrs.md` (architect, to come; small: the part search reuses the region plan of `0035`)
PR: TBD
