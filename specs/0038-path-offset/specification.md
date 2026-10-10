# Path offset: Outset and Inset by a typed distance, with a live preview

Status: Ready (2026-10-10: `adrs.md` and the UX notes exist; the kernel question is decided, no customer question is open; criteria 11 and 19 and the miter definitions reworded on 2026-10-10 to what the library does, see `adrs.md`)
Priority: Should
Origin: Customer (Inset and Outset, request of 2026-10-09). R-EDIT-007 (outline/offset) existed as my Proposal since the first requirements; the customer has now asked for it. The preview, the copy-not-replace rule, the open-path rule and the defaults are my proposals, marked in "Decided by the product owner".

## User value

As a maker I want to make a bigger or smaller copy of a shape at an exact distance (for example 2 mm outside a part for a kiss-cut border, 0.15 mm inside it for a kerf allowance, 3 mm around a line for a cut-out slot), and to see the result before I accept it, so that borders, seam allowances, inlays and engrave-versus-cut separations are one typed number and not a hand-drawn second outline.

**Reference tools.**

- Inkscape: Path > Outset and Inset move the outline by a fixed step that is set in Preferences (Behaviour > Steps, default 2 px, in pixels, not in the command); Dynamic Offset and Linked Offset are two more commands with different behaviour; the result replaces the original; no preview. As I understand them.
- Illustrator: Object > Path > Offset Path, a dialog with Offset, Joins (Miter, Round, Bevel), Miter limit and a Preview check box; it adds a new path.
- LightBurn: Offset Shapes with Distance, Corner style (Round, Sharp, Bevel), Direction (outward, inward, both) and a "Delete original" check box; the closest model for a laser maker.
- Affinity: the Contour tool.

What we do better: the distance is in the command and in mm, the result is shown in blue over the unchanged original before it is accepted, the original is kept by default (a mistake is one Delete away, and `0020` undo takes the whole step back), and the rules for corners, open paths, holes and vanishing shapes are written down with numbers.

## Decided: the kernel, no new dependency (lead, 2026-10-10)

Offset uses the library the Boolean operations already use: `i_overlay` `=9.0.1` (MIT OR Apache-2.0), exact pin as in ADR 0003 §3. Its outline offset covers closed shapes (outset and inset, with miter, bevel and round joins) and its stroke offset covers open paths (round, flat and square ends, the same joins). So:

- **No new dependency**, no `Cargo.toml` change for this slice, no `cargo deny` change.
- The plan of ADR 0003 §4 (expand each contour with kurbo and union the pieces) and the open check of its 2026-10-09 amendment are closed by this. The architect records it in `adrs.md`; it is not a customer question.
- If a join, an end or an inset behaviour that the criteria below need is not what the library gives, the architect tells the product owner and the criterion is changed on purpose. It is not weakened silently.
- The same library has a stroke offset whose width varies along the path (round joins and ends only). It is not used here; `0033-stroke-brushes` may start from it.

## What the result is: a polyline

The offset runs on straight segments, as the Boolean operations of `0016` do. For curves this means:

- A curved operand (a circle, an ellipse, a Bézier path, a rounded rectangle) is first flattened to straight segments within 0.01 mm (`0016` criterion 25) and then offset. The result of a curve is **a polygon with many short segments, never a Bézier**. The Node tool shows corner nodes without handles.
- A Round join or end is an arc, also flattened within 0.01 mm. With the Round join the result is therefore within 0.02 mm of the exact offset of the original curves (0.01 mm for the input, 0.01 mm for the arcs). With Miter or Bevel the chord corners of a flattened curve are joined as corners, which adds about 0.01 mm x |d| / R (R the curve's radius) to that distance; the result is still exact to 0.01 mm against the offset of the flattened operand (criterion 11). For an operand with straight edges only, the straight parts of the result are exact to the 0.001 mm grid.
- The node count grows with the size and curvature of the operand and is bounded by criterion 11.
- The original stays as it is, so its curves are not lost. A maker who wants a smooth bigger circle draws a bigger circle.
- Refitting curves to the result is a follow-up, as for the Boolean operations (`0016`).

## Words used below

- **Operand:** a selected closed ordinary path, compound path, rectangle, ellipse, polygon or star, or an open ordinary path. The **region** of a closed operand is the area its outline(s) enclose under the nonzero rule, as the canvas paints a fill, whether or not Fill is on (`0016` criteria 4 and 5). For an open path the region to grow is the path itself, a line of no area.
- **Distance d:** signed, in millimetres. Positive grows the region outward (Outset), negative shrinks it inward (Inset). The sign is the side; there is no separate direction choice (Question 5).
- **Join:** how a corner is treated where the offset outline opens a gap (at convex corners when growing, at concave corners when shrinking): **Round** (an arc of radius |d|), **Miter** (the two edges extended to their meeting point, clipped at 4 x |d| from the corner when the point is farther away), **Bevel** (a straight cut across the gap). Default Round.
- **Miter limit:** fixed at 4, as SVG's: a miter is kept while its tip is at most 4 x |d| from the corner, which is a corner angle of at least about 28.96 degrees. A sharper corner is **clipped at the limit**: the tip is cut off by a straight edge whose two nodes lie 4 x |d| from the corner, symmetric about the corner's bisector (SVG 2's `miter-clip`, what the offset library does; decided 2026-10-10, `adrs.md`). It is not bevelled back to the offset edges. Not a setting.
- **Cap:** how the ends of an open path are finished when it is grown: **Round**, **Flat**, **Square**. Default Round. Only open paths have caps.
- **Kernel tolerance:** 0.01 mm, as `0016`.

## Acceptance criteria

### Entry and the preview

1. Given the Select tool and a selection of one or more operands, when the maker activates **Offset**, then an Offset entry opens with a Distance field (a typed or dragged value in mm, positive outward and negative inward, typed range −1000 to 1000, drag range −50 to 50, default 1 mm on first use and then the last value used in this session), a Join choice (Round, Miter, Bevel), a Cap choice shown only when an open path is selected, and the buttons Apply and Cancel. The entry is not a popup over other panel content: it is a section at the top of the Properties panel, opened by a text button "Offset" in the Select bar (UX notes). Given no operand is selected, Offset is not offered.
2. Given the entry is open and the Distance is a valid non-zero number, then the result of Apply is shown as a blue (`--accent`) 1.5 px hollow outline over the canvas, with the original operands unchanged and black (the blue-new/black-old convention of `0009`). The preview shows every outline of the result, holes included; it is never stored, and a filled result shows no fill preview. It follows a dragged Distance at one update per animation frame, and a typed Distance after Enter or Tab (no preview while typing, as `0017` criterion 42). A change of Join or Cap updates it the same way.
3. Given an operand of 200 nodes, then the preview updates within 50 ms of each change of the Distance, Join or Cap on the reference desktop in a release build. A larger operand may update later, but the pointer and the field never block, and what Apply writes is exactly what the last preview showed.
4. Given the entry is open, when the maker presses Escape, Cancel, changes the tool or the selection, or presses on empty canvas, then the entry closes, the preview disappears and nothing is written. Escape is the first step of the cascade of `0010` criterion 42 (an open entry handles it).
5. Given Apply (button or Enter in the Distance field with the value committed), then the result of criteria 6 to 14 is written as one commit, the entry closes, and the results are the selection. A Distance of 0 closes the entry and writes nothing.

### The result

6. Given a closed operand and d, then the result is a new object whose region is the operand's region grown by d (d > 0: every point within d of the region) or shrunk by |d| (d < 0: every point of the region farther than |d| from its boundary), with the Join rule at corners. Holes behave as parts of the boundary: growing a ring shrinks its hole, shrinking it grows the hole. A result with one outline and no hole is an ordinary closed path, otherwise one compound path (`0016` criterion 20).
7. Given a closed operand that is shrunk so far that it disappears, or in parts, then the parts that vanish are gone from the result. Given every part of every operand vanishes, then Apply is refused with "Inset is empty: nothing is left at 10 mm. Nothing was changed." (the number is the typed distance), a chip as `0016` criterion 17, nothing written. Given some operands vanish and others do not, then the survivors get their results and the notice says "1 of 3 shapes vanished".
8. Given an open path and d > 0, then the result is a closed outline of all points within d of the path: the path's both sides at distance d, joined by the Join choice at its corners and finished by the Cap at its ends. It is one closed path (a compound path if the path crosses itself and leaves a hole). Given an open path and d < 0, then Apply is refused with "Inset needs closed paths. 1 of 2 selected objects is open. Nothing was changed.", the open ones outlined in red as `0016` criterion 15.
9. Given a self-crossing operand, then its region is the area the canvas paints (nonzero), as `0016` criterion 7, and the result is a clean outline without self-crossings.
10. Given the result, then the **original operands are kept** and the results are new objects placed directly above their originals in the stacking order (each result above its own operand). Each result's complete style is a copy of its operand's style. Several operands give one result each; they are not merged with each other. (Question 2.)
11. Given the result geometry, then it is a polyline: straight segments only, every node is a Corner node without handles, the outline lies within 0.01 mm of the exact offset of the flattened operand with the chosen Join and Cap (with the Round join, and so for every straight-edged operand with any Join, this also puts it within 0.02 mm of the exact offset of the original curves; with Miter or Bevel on a curved operand only the bound to the flattened operand holds, see "What the result is"), no two consecutive nodes are closer than 0.001 mm, no node lies on the straight line between its neighbours (`0016` criterion 24), and a Round join or cap has between one and two times the fewest segments that keep every chord within 0.01 mm (the node budget of `0016` criterion 26). Winding follows `0016` criterion 41. The result is rotation 0 (`0016` criterion 27).
12. Given the same operands and entry values, then the result is node for node the same however they were selected and on every platform to 1e-6 mm (`0016` criterion 43).
13. Given a successful Apply, then exactly one commit is made, stored as `offset_path`, that adds all results together, atomically (`0016` criterion 28); and a notice names what happened ("Offset: added 2 shapes, 5 mm outward.", `role="status"`, 3 seconds, as `0016` criterion 29).
14. Given an operand that is a group, then Apply is refused with "Offset does not work on groups. Ungroup first. Nothing was changed." (`0023-groups` criterion 28). A coordinate that is not finite, an operand beyond `MAX_COORDINATE_MM`, or a result beyond it, is refused as `0016` criterion 17a. An operand that encloses no area and is closed (all nodes on one line) is refused as `0016` criterion 16.

### Tests

Areas are measured on the result polygon; the tolerances allow for the flattening of arcs (criterion 11).

15. A square of side 20 mm with its corner at (0, 0): Outset 5 mm with Miter gives a square from (−5, −5) to (25, 25) of 4 nodes. With Round the area is 400 + 4 x 20 x 5 + π x 25 ≈ 878.54 mm² (within 0.3 mm²); with Bevel it is 400 + 400 + 4 x 12.5 = 850 mm² (exact to 0.01 mm²). Inset 5 mm gives the square (5, 5) to (15, 15), area 100 mm², with every Join. Inset 10 mm is refused as empty (the square collapses to a line; a result of no area counts as empty), and Inset 9.99 mm gives a square of side 0.02 mm.
16. A ring (a disc of radius 20 mm minus a concentric disc of radius 10 mm, a compound path): Outset 2 mm Round gives outer radius 22 and hole radius 8 (area π(484 − 64) ≈ 1319.5 mm² within 3 mm²); Inset 2 mm gives outer 18 and hole 12 (area π(324 − 144) ≈ 565.5 mm² within 3 mm²). Inset 5 mm: hole radius 15, outer radius 15 means the ring vanishes (refused as empty).
17. An open line from (0, 0) to (20, 0): Outset 5 mm gives a closed path of area 20 x 10 + π x 25 ≈ 278.54 mm² with the Round cap (within 0.3 mm²), 200 mm² with the Flat cap (the rectangle (0, −5) to (20, 5)), and 300 mm² with the Square cap (from (−5, −5) to (25, 5)). Inset of the line is refused.
18. A concave corner: an L-shaped closed path, Outset 2 mm: the outer convex corners follow the Join choice; the one inner reflex corner stays a sharp corner, since the offset lines overlap there and no gap opens. Inset 2 mm: the reflex corner follows the Join choice (a gap opens), the convex corners stay sharp.
19. Miter limit: a square (all corners 90 degrees), Outset 5 mm, Miter: the corner tips lie 5 x √2 ≈ 7.07 mm from the original corners, below the limit of 4 x 5 = 20 mm, so they are sharp points (criterion 15). A triangle with one corner of 20 degrees, Outset 5 mm, Miter: the tip would lie 5 / sin(10°) ≈ 28.8 mm from that corner, above 20 mm, so the corner is clipped at the limit: it gets two nodes, each 20 mm (4 x 5 mm) from the original corner within 0.01 mm and symmetric about its bisector, and no node 28.8 mm from it. The other two corners (80 degrees each) stay sharp.
20. Curves are polylines: a circle of radius 20 mm (4 Bézier nodes), Outset 2 mm Round: the result is one closed path with no handles on any node, every node lies within 0.02 mm of the circle of radius 22 mm, and the area is π x 22² ≈ 1520.5 mm² within 3 mm² (the polygon is inscribed). The original circle still has its 4 nodes and its handles.

## Out of scope

- **Replacing the original** and a "Delete original" switch (Question 2).
- **Several offsets at once** (a count, concentric copies, hatching) and **both directions** at once.
- **A live offset that stays linked** to its source (Inkscape's Dynamic and Linked Offset, Affinity's Contour).
- **Offset by a variable distance**, a miter limit setting (fixed at 4), caps for closed paths.
- **Offsetting strokes** (a stroke's width is not part of any region); a "stroke to path" feature is separate.
- **Keeping curves as curves** in the result (refitting; see "What the result is").
- **Groups** (criterion 14), **text**, **images**.
- **V-carving and CNC tool-radius compensation** (R-MFG-CNC-001, R-MFG-CNC-002): these use offsetting in the job generator and are specified with the CNC slices. The kernel routine written here is meant to serve them, which the architect considers.
- **Document units other than mm** in the Distance field (`0017` criterion 48; one follow-up for all fields).
- **Undo and redo** (`0020`).

## Open questions

Each has a default; nothing blocks. The old Question 1 (the kernel) is decided, see "Decided: the kernel".

1. **Keep or replace the original.** *A (default):* keep, add a copy above it (R-EDIT-007 says "copy"). *B:* a "Delete original" switch in the entry, off by default (LightBurn). *C:* replace always (Inkscape). Recommendation: A now, B as a small follow-up.
2. **Round as the default join.** Default Round (Illustrator's is Miter, LightBurn's Round). Round is the safe choice for a cut line (no spikes). Say if you prefer Miter.
3. **Miter limit of 4.** Default; fixed, as SVG's.
4. **Typed range ±1000 mm and drag range ±50 mm.** Defaults, chosen like stroke width (`0017`, 1000 typed, 20 drag).
5. **Name.** Default: "Offset" for the command with Outset and Inset being the sign of the distance. Option: two buttons "Outset" and "Inset" that open the entry with the sign preset. Recommendation: one command, because the sign is visible and changeable in the preview.

Decided by the product owner (change if you disagree): the original is kept and results go above it; an Inset that removes everything is refused; open paths grow into a closed outline, never shrink; no merging of results; the Offset entry remembers its values for the session only; commit label `offset_path`; the 4 x limit; the areas and tolerances of the tests.

## UX notes

By `ux-engineer`, 2026-10-10. Numbers and rows are in `docs/design-system.md`: "Command with parameters", "Offset section", "Select bar layout", "Live preview outline", "Action notice", "Refusal outline". Reference tools: Illustrator's Offset Path dialog (distance, joins, miter limit, Preview check box), LightBurn's Offset Shapes, Inkscape's fixed-step Inset and Outset (no preview). We keep Illustrator's and LightBurn's choices and drop the dialog and the Preview check box: the preview is always on.

### Where it lives: a Select bar button that opens a panel section

This is the first command with a value, so it needs a place for Distance, Join, Cap, Apply and Cancel. Decision: the entry is the text button **Offset** in the Select bar (shown while the selection holds an operand, hidden otherwise, which is criterion 1's "not offered"; hidden for a group alone), and it opens a **section at the top of the Properties panel**, above the Style area, with focus in Distance. The criterion 1 wording that left this open ("a chip at the selection box ... or a group in the Select bar") is replaced by this.

Rejected: a rail button (the rail is full, and Offset would be dimmed with nothing selected where the spec says not offered); a chip at the selection box as in `0010` (a chip holds one or two fields; this has five controls and must not cover the preview); a group inside the Select bar (the bar wraps to three rows at 800 x 600 and would hold a field that previews while the bar's own fields do not); a dialog or popover (the panel has none, and a dialog covers the canvas where the preview is drawn). The cost is eye travel from the bar to the right panel; the section opening and the focus move are the cue, and the preview is on the canvas, which the panel does not cover.

### The section

Rows of 28 px, 8 px apart, the panel's own components (row "Offset section"):

| Row | Control | Notes |
|---|---|---|
| Title | "Offset", 12 px semibold | `role="group"` named "Offset" |
| Distance | the panel's value field, mm | typed -1000 to 1000, dragged -50 to 50, 1 mm first, then the last value of the session; a reserved 16 px caption below it |
| Join | three icon items, 40 px each: Round, Miter, Bevel | the glyphs of the stroke Join; default Round |
| Cap | three icon items: Round, Flat, Square | only while an open path is selected; default Round |
| Buttons | Apply (filled, `--toolbar-icon-active-bg`, white label), Cancel (outline), 72 px wide each | last in Tab order |

188 px high with Cap, 152 px without. It pushes the Style rows down while open, which is the one deliberate shift: the maker entered a mode, and the section is the first thing in the panel. The panel scrolls if the window is short.

**The sign is the side.** The one command name "Offset" hides which way a minus goes, so the caption under Distance reads "Outset" for a positive value and "Inset" for a negative one (nothing at 0). The same slot carries the reason when the result would be empty or refused: "Nothing is left at -10 mm." (criterion 7) or "Needs closed paths: 1 of 2 selected is open." (criterion 8). It is a polite live region, muted, with the `--field-invalid` colour and an alert glyph for a refusal, and it has a fixed height so the section does not jump.

### Opening, previewing, applying

- Pressing Offset opens the section (expanding the panel if collapsed, activating the Style tab if another tab shows, as Shift+Ctrl+F does) and **shows the preview at once** with the remembered Distance, so the maker sees a result before touching anything. The button shows the open ground and `aria-expanded="true"`; pressing it again closes the section.
- The preview is the blue hollow outline of the result over the unchanged originals (row "Live preview outline"), holes included, no fill. A dragged Distance previews once per frame; a typed value previews after Enter or Tab (criterion 2). **Enter in Distance works in two steps:** the first commits a typed value and shows its preview, the second applies (criterion 5, "with the value committed"). Join and Cap changes preview at once.
- While a large operand's preview is being computed the caption reads "Updating..." and Apply is ignored until the preview for the current values is on screen, so what is written is exactly what was last shown (criterion 3). For 200 nodes this lasts under 50 ms and is never seen.
- Close without writing: Escape (the first step of the cascade, criterion 4), Cancel, another tool, another selection, a press on empty canvas, or a Distance of 0 with Apply. Apply by mouse returns focus to the canvas; by key it returns to the Offset button.
- After Apply the selection is the new shapes, the section closes, and the notice "Offset: added 2 shapes, 5 mm outward." (3 s, `role="status"`) sits 4 px under the Offset button. A refusal (criteria 7, 8, 14) is an alert chip 4 px under Apply, 8 s; the section stays open so the value can be changed.

### States

| Selection | Offset button | Section |
|---|---|---|
| Nothing, or only groups | not in the bar | cannot open |
| One or more closed shapes, compound paths, primitives | shown | Join; no Cap |
| An open path (alone or with closed shapes) | shown | Join and Cap; a negative Distance refuses the open ones (caption, then Apply) |
| A group together with other objects | shown | Apply refuses with the group sentence (criterion 14), the group outlined |
| Another tool than Select | the bar is not shown | an open section closes |

### Keyboard, accessibility, contrast

Tab reaches Offset in the Select bar; Space or Enter opens; focus lands in Distance; Tab order is Distance, Join, Cap, Apply, Cancel; arrows move inside Join and Cap (one Tab stop each, `role="radiogroup"`); Escape closes. Labels are visible ("Distance", "Join", "Cap"); the accessible name of the field is "Distance, millimetres". The value field's drag has the keyboard route of the panel's fields (arrows, Shift ten times, Ctrl a tenth). Apply white on `--toolbar-icon-active-bg` is 4.5:1; the outline Cancel and the muted caption (`--panel-muted-fg` 5.0:1) pass. Targets are 28 px high or more. Colour is never the only cue: the preview is also described by the caption, and a refusal has the alert glyph and the sentence.

### At 800 x 600, large documents

The section is 188 px of a panel about 546 px high; the Style rows below it scroll. The Select bar gains the Offset button, which comes last and wraps by whole groups; the number of rows at the narrowest width is measured at build (the settings group alone nearly fills the 356 px row, so Offset is likely to sit on a second row for any selection). The preview of 2,000 nodes may lag; the field and the pointer never block.

### Questions for the customer

1. **Keep or replace the original (Question 1).** The design works with A (keep, copy above). With B later, the switch is one more row in the section ("Delete original", off).
2. **Name (Question 5).** One command "Offset" with the sign as side, made readable by the caption. Two buttons would cost a second bar button for nothing.

## Links

Requirements: R-EDIT-007 (`docs/requirements.md`)
Builds on: `specs/0016-boolean-operations/` (kernel, compound paths, region, refusals, notices, determinism; must be merged first), `specs/0009-unified-object-editing/` (blue-new/black-old), `specs/0010-edit-interaction-polish/` (entry chips, Escape cascade), `specs/0017-style-panel-rework/` (value fields)
Related: `specs/0023-groups/` (groups refused), `specs/0037-fracture-and-flatten/`, `specs/0033-stroke-brushes/` (variable-width stroke offset), `docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md` §3, §4
ADRs: `adrs.md` (architect, to come; ADR 0003 §4 amendment: `i_overlay` offset, no new dependency)
PR: TBD
