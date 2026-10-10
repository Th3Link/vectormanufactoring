# Fracture and Flatten: cut overlapping shapes into pieces, or remove the hidden parts

Status: Draft (the criteria are complete and testable; no customer question blocks (Flatten means trim to the visible part, decided 2026-10-10); becomes Ready when `adrs.md` and the UX notes exist, `CLAUDE.md` §4)
Priority: Should
Origin: Customer asked for "Fracture" (cut overlapping shapes into non-overlapping pieces, like Affinity's Divide) and "Flatten" (request of 2026-10-09), and left the exact meaning to me. **The definitions below are my proposals** (Question 1 asks about Flatten in particular).

## User value

As a maker I want to take a stack of overlapping shapes and either (Fracture) cut it into the separate pieces the overlaps make, or (Flatten) trim every shape to what is actually visible, so that the laser never cuts the hidden part of a lower shape twice, a multi-part design becomes real separate parts, and an inlay or marquetry pattern can be cut piece by piece from the shapes I drew over each other.

Both are one idea. Take all outlines of the selected shapes and cut the plane along them. Each resulting **cell** is covered by some of the shapes; the **owner** of a cell is the topmost shape covering it.

- **Fracture** keeps every cell as its own object, painted with its owner's style. Two overlapping circles give three pieces: the left crescent, the lens, the right crescent. Nothing is lost and nothing is merged.
- **Flatten** gives every shape only the cells it owns, joined into one object per shape. The picture on the canvas does not change (fills), but nothing overlaps any more. Two overlapping circles give two objects: the lower one as a crescent, the upper one whole. A shape that is completely hidden has no cell and is removed.

**Reference tools.** Illustrator's Pathfinder has Divide (our Fracture, except that it keeps the pieces grouped and fills them), Trim (our Flatten, except that it also drops strokes) and Merge, in a panel of ten small icons. Affinity's Divide cuts all selected shapes into pieces. Inkscape has Division and Cut Path for exactly two paths (the top one cuts the bottom one and is used up) and nothing that does Fracture or Flatten on a stack; makers use Division repeatedly or a chain of Differences. LightBurn welds and subtracts but does not divide a stack. Where we do better: both commands take any number of shapes in one step, are named in plain words with a tooltip that states the rule, an untouched shape stays exactly as it was (its curves, ids and style), and the notice says how many pieces came out and what was removed.

## Words used below

- **Operand:** a selected closed ordinary path, compound path, rectangle, ellipse, polygon or star. **Region** of an operand: the area its outline(s) enclose under the nonzero rule, as the canvas paints a fill, **whether or not its Fill is on** (`0016` criteria 4 and 5). Paint plays no part.
- **Cell:** a connected area of the plane covered by exactly the same non-empty set of operands.
- **Owner** of a cell: the operand with the highest stacking position among those covering it. Stacking order, never selection order (`0016` criterion 9).
- **Piece:** a cell turned into an object. A cell with holes (another cell inside it, or a hole of an operand) is a compound path (`0016`).
- **Kernel tolerance and grid:** 0.01 mm and 0.001 mm, as `0016` (criteria 39, 25).

## Acceptance criteria

### Entry points and refusals

1. Given any state of the app, then Fracture and Flatten are two commands whose buttons follow the rules of `0016` criteria 1, 1a, 2 and 3 for the Boolean buttons (always rendered; dimmed when fewer than two objects are selected or the tool is not Select; still focusable; a three-line tooltip; no shortcut; the tool never changes). They are the sixth and seventh buttons of the Boolean toolbox (Fracture, then Flatten), which takes the same selection under the same dimming rule. Tooltips: "Fracture" / "Cuts overlapping shapes into pieces." / "Each piece keeps the style of the shape on top. Replaces the selection. No undo yet." and "Flatten" / "Trims each shape to what shows." / "Hidden shapes are removed. Fill does not matter. Strokes follow the trim. No undo yet."
2. Given an operand that is an open path, then the command is refused with the notice of `0016` criterion 15 ("Fracture needs closed paths. 1 of 3 selected objects is open. Nothing was changed.") and the red outline on the offender. An operand that encloses no area, a coordinate that is not finite or beyond `MAX_COORDINATE_MM`: refused as `0016` criteria 16 and 17a. An operand that is a group: "Fracture does not work on groups. Ungroup first. Nothing was changed." (`0023-groups` criterion 28).
3. Given no two operands overlap (their regions share no area; touching along an edge or at a point is not an overlap), then the command is refused with "Fracture: the selected shapes do not overlap. Nothing was changed." (Flatten likewise) and nothing changes.
4. Given the result would have more than 5,000 objects, then the command is refused with "Fracture would make more than 5000 pieces. Nothing was changed."
5. Given a refusal, then everything of `0016` criterion 18 holds (no dialog, buttons stay usable, focus rules).

### Fracture

6. Given two or more operands that overlap somewhere, when the maker activates Fracture, then the operands that take part in an overlap are removed and replaced by one object per piece. An operand that overlaps no other operand is **left exactly as it is** (same object, same id, curves kept) and stays selected.
7. Given criterion 6, then the pieces are exactly the cells of the participating operands: their areas add up to the area of the union of the participating regions (within the bound of `0016` criterion 14), no two pieces share area, and the cells' set of covering operands differs from piece to piece. Test: two squares of side 20 mm, the second offset by (10, 10) mm, give three pieces: the first square without the overlap (an L, area 300 mm², 6 nodes), the overlap (10 x 10 mm, area 100 mm², 4 nodes), the second square without the overlap (an L, 300 mm²). Test: a disc of radius 20 mm and a disc of radius 10 mm in the same centre give two pieces, a ring (a compound path, area π(400 − 100) mm²) and a disc of radius 10 mm. With the small disc on top, the ring has the large disc's style and the inner disc the small disc's. With the small disc below, both pieces have the large disc's style (the picture is unchanged, cut in two).
8. Given criterion 6, then each piece's complete style is a copy of its owner's style, and the piece sits at its owner's place in the stacking order (consecutively with that owner's other pieces), so objects that were not selected and lie between operands keep covering and being covered as before. Pieces of one owner are ordered by the position of their first node, left to right then top to bottom.
9. Given criterion 6, then all pieces and all untouched operands are selected afterwards, the tool is unchanged, one commit is made, stored as `fracture`, atomic as `0016` criterion 28, and a notice names the counts ("Fracture: 3 objects became 7 pieces. No undo yet.").

### Flatten

10. Given two or more operands that overlap somewhere, when the maker activates Flatten, then each operand O that is covered by an operand above it is replaced by one object holding its **visible region**: the region of O minus the union of the regions of all operands above O. A visible region of several separate areas, or with holes, is one compound path. An operand with nothing above it that overlaps it is **left exactly as it is** (same object, same id, curves kept, same style). Test: the two squares of criterion 7: the first becomes the L of 300 mm² (6 nodes, one closed path); the second is untouched.
11. Given criterion 10, then an operand whose visible region is empty is removed, and the notice says so. Test: a disc of radius 10 mm below a disc of radius 20 mm in the same centre: the small disc is removed and the large one is untouched; the notice reads "Flatten: 2 objects became 1. 1 hidden object was removed. No undo yet."
12. Given criterion 10, then each result keeps its operand's complete style and its place in the stacking order, so the picture of fills does not change except where noted in criterion 13. The result is selected together with the untouched operands.
13. Given a trimmed operand with a stroke, then the stroke is drawn along its new outline, so a stroke that was hidden under an upper shape is now visible along the trim edge (and stroke is not part of the visible region, as it is not part of any region in this product). The Flatten tooltip is the only warning; stroke-heavy drawings are better fractured or flattened after the stroke is converted (a later feature).
14. Given criterion 10, then one commit is made, stored as `flatten`, atomic, with the notice of criterion 11's form.

### Result geometry and robustness

15. Given an operand with curves, then a trimmed or cut part of it is straight segments within 0.01 mm of the true result and with the node budget of `0016` criteria 25 and 26 (this is the cost of using the boolean kernel; Combine, Break apart, Split and Cut keep curves). All nodes of a result are Corner nodes without handles, no two consecutive nodes are closer than 0.001 mm, and no node lies on the straight line between its neighbours (`0016` criterion 24). Winding follows `0016` criterion 41.
16. Given the same objects at the same stacking positions, then the result is node for node the same however they were selected, and the same on every platform to 1e-6 mm (`0016` criterion 43).
17. Given the fixtures (in `tests/fixtures/`, golden): (a) 20 squares of side 10 mm at x = 5 i for i = 0 to 19, each overlapping its left neighbour by 5 mm, (b) 2,000 such squares, (c) the fixtures (a) to (i) of `0016` criterion 40, then each command completes in under 2 s without panic or hang and gives a valid result or one of the refusals. Expected for (a), with the highest index on top: Fracture gives 21 pieces of 5 x 10 mm (the 5 mm columns, owner the upper square; the left-most and right-most columns have one covering square); Flatten gives 19 objects of 5 x 10 mm and the top square unchanged (10 x 10 mm). Expected for (b): Fracture 2,001 pieces.
18. Given a result, then the Node tool edits each piece like any path; a piece with holes is a compound path whose nodes the Node tool does not edit yet (`0016` criterion 38).

## Out of scope

- **Merging pieces of equal style** (Illustrator's Merge) and **joining adjacent pieces into one**: Union (`0016`).
- **Keeping curves** in the result. Refitting curves after a boolean is a follow-up of `0016`.
- **Paint-aware behaviour**: a shape with Fill None hides what is below it, as the Boolean operations treat it (Question 2).
- **Strokes**: a shape's stroke is not part of its region; there is no "outline stroke" (stroke to path) here.
- **Groups** (`0023-groups` criterion 28), **open paths**, text.
- **Using one shape as a cutter that is used up** (Inkscape's Division): fracture, then delete the pieces not wanted.
- **Hover preview of the result** (see "Specified but not built" in `specs/README.md`).
- **Undo and redo** (`0020`).

## Open questions

Each has a default; nothing blocks.

1. **What Flatten means. Decided by the customer on 2026-10-10: A.** *A (default):* trim every shape to its visible part (criterion 10). *B:* "flatten hierarchy": ungroup everything recursively (needs `0023-groups`). *C:* merge shapes of the same colour that touch. Please say if you meant B or C; A needs no groups and is what a laser maker asks for. If you meant Illustrator's "Flatten Transparency", that has no meaning here (no transparency effects exist).
2. **Fill None hides.** *A (default):* an operand hides whatever is below it whatever its paint, because regions are geometry (`0016`). *B:* operands whose Fill is None do not hide anything. Reason for B: a drawing of fill-less cut lines would not lose its lower lines. Reason for A: one rule for all geometry commands, and the lower line is a second cut of the same area. Recommendation: A, since Flatten is a geometry command.
3. **Order of pieces in the stacking order (criterion 8).** Default: at the owner's place. Option: all at the topmost operand's place.
4. **Limit of 5,000 objects.** Default as written.
5. **Names.** Default: "Fracture" and "Flatten" (your words). Option: "Divide" and "Trim" (Illustrator's), which say more.

Decided by the product owner (change if you disagree): cells and owners as defined; untouched operands stay exactly as they were; hidden operands are removed with a notice; commit labels `fracture` and `flatten`; the 5,000 limit; piece order by first node.

## UX notes

By `ux-engineer`, 2026-10-10. Numbers and rows are in `docs/design-system.md`: "Tool rail architecture", "Boolean toolbox", "Boolean tooltip", "Command glyphs", "Action notice", "Refusal outline". The rail rules are those of the Boolean toolbox and are not repeated here. Reference tools: Illustrator's Pathfinder panel holds Divide and Trim in the same panel as Unite and Minus Front; Affinity's Divide sits with Add and Subtract. Neither separates them from the Boolean operations.

### Placement: the Boolean card, buttons six and seven

Fracture and Flatten join the Boolean card, below Reverse difference: **Union, Difference, Intersection, Exclusion, Reverse difference, Fracture, Flatten**. Reasons: they take the same selection (two or more closed shapes, Select tool) and so dim and enable exactly as the five already there; they work on areas and not on a path's own nodes; and their results are straight segments, like the Boolean operations (criterion 15), where the Path card keeps curves. The card grows from 224 px to 312 px; the toolbar keeps the accessible name "Boolean operations". With this release the Boolean card has to be in column B (below Path, where it ends 512 px below the viewport top); if the Group release or the Pencil release has not moved it yet, this release does (row "Tool rail architecture"). In column A it would end 556 px down a 546 px viewport. The criterion 1 sentence "Where they sit is the ux-engineer's decision" is replaced by this placement.

### States

| Situation | Fracture, Flatten |
|---|---|
| Fewer than two objects selected | dimmed, "Select two or more closed objects." |
| Another tool than Select | dimmed, adds "Use the Select tool." |
| Two or more objects, any kinds | enabled |
| An open path, a group, an object without area | enabled; pressing refuses with the sentences of criterion 2, the offenders in the red outline |
| Closed shapes that do not overlap | enabled; pressing refuses (criterion 3), no outline: whether shapes overlap needs the kernel, so it is not a dimmed state |

Dimming is the rail's: `aria-disabled="true"` at 40 % opacity, focusable, tooltip stays open, activation does nothing.

### Telling the two apart, and from Union

Rule lines: Fracture "Cuts overlapping shapes into pieces." / Flatten "Trims each shape to what shows." (both under 45 characters). The result on the canvas looks the same for fills in both (the picture does not change); what differs is the object count: Fracture ends with more objects than it started with, Flatten with the same or fewer. The notices say it: "Fracture: 3 objects became 7 pieces." and "Flatten: 2 objects became 1. 1 hidden object was removed." The glyphs: Fracture is three solid pieces separated by gaps, Flatten is the upper square whole and the lower one trimmed to its visible L, two interlocking solid pieces with a gap along the trim edge. Union has no gap at all; Exclusion has a hole where the others have gaps.

### Tooltips

Fracture: "Fracture" / "Cuts overlapping shapes into pieces." / note "Each piece keeps the style of the shape on top. Replaces the selection." Flatten: "Flatten" / "Trims each shape to what shows." / note "Hidden shapes are removed. Fill does not matter. Strokes follow the trim." (two lines; the note carries the three surprises: removal, fill, and stroke, which criterion 13 says the tooltip must warn about). When dimmed or open paths are selected the notes of the Boolean tooltip row apply first. The criterion 1 tooltip texts are replaced by these (the old rule lines were 59 and 87 characters and would have wrapped; the stroke warning was missing). The hover preview is out of scope here, so there is no preview line in the note.

### Interaction

One press, no dialog. All pieces and untouched shapes are selected afterwards; the tool is unchanged; a notice beside the Boolean card (level with Union) names the counts, 3 s, or 5 s above 70 characters, `role="status"`. Pieces of one owner sit consecutively at the owner's place in the stacking order (criterion 8), so what the maker did not select stays above or below as before. A result piece with holes is a compound path, which the Node tool does not edit yet (criterion 18, row "Compound path in the UI"). Fill None hides what is below (Question 2, default A): a drawing of fill-less cut lines loses its lower lines under Flatten, and the notice "1 hidden object was removed." makes that visible and not silent.

### Keyboard, accessibility, contrast

No shortcut. One Tab stop for the toolbox, Up and Down move, Space or Enter presses, focus stays on the button after a key press and returns to the canvas after a mouse press. The accessible names are "Fracture" and "Flatten"; the rule line is the description. Refusal text `--field-invalid` 6.5:1 on its ground, the red outline 5.3:1 on the canvas, and the sentence names the count, so colour is not the only cue. Success text `--toolbar-icon` on `--toolbar-bg` 8.3:1.

### At 800 x 600, large documents

Column B is 500 px of 546 px with both new buttons (34 px clear); nothing else in the viewport changes. 2,000 overlapping squares (fixture b) give 2,001 selected pieces: the group box shows, member boxes do not (above 500, row "Member box"). The kernel call runs on the UI thread, so the toolbox is `aria-busy` with the wait cursor, as for the Boolean operations (`docs/technical-debt.md`); both commands must finish within 2 s (criterion 17).

### Questions for the customer

1. **Names (Question 5).** Default stays "Fracture" and "Flatten" (his words); the rule lines say what each does, which "Divide" and "Trim" would have said by name. Say if you want Illustrator's names.
2. **The Boolean card now holds seven buttons and two kinds of command** (area combination, and cutting or trimming a stack). *Default:* one card, as Illustrator and Affinity do; the alternative is a separate three-button card, which does not fit column B.

## Links

Requirements: R-EDIT-023 (`docs/requirements.md`); related R-EDIT-003
Builds on: `specs/0016-boolean-operations/` (the kernel, compound paths, region, refusals, determinism; must be merged first)
Related: `specs/0035-combine-and-break-apart/`, `specs/0036-cut-at-crossings/`, `specs/0048-split-compound-path/`, `specs/0038-path-offset/`, `specs/0023-groups/`
ADRs: `adrs.md` (architect, to come; whether Flatten and Fracture are built from the pairwise kernel operations of `0016` or from a planar-arrangement routine)
PR: TBD
