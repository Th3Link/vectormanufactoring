# Segment drag bending: drag a line or curve segment of a path to bend it

Status: Ready (`adrs.md` by the architect and the UX notes by the ux-engineer exist, both 2026-10-10; their requested changes are applied below)
Priority: Should
Origin: Customer (the request: click a segment in the Node tool and drag it to bend it, as in Inkscape). The shape rule, the 3 px dead zone, the hover look, Shift and the preview are my proposals, marked in "Decided by the product owner" and "Proposals".

## User value

As a maker I want to grab a line or a curve of a path with the Node tool and drag it, so that the line turns into a curve or the curve changes shape right where I pull, without first adding a node or dragging handles one by one.

**Reference tools.** Inkscape's Node tool does this: dragging a segment bends it, and the two handles of the segment move so that the curve follows the pointer. Illustrator (Direct Selection) and Affinity (Node tool) drag a segment too. What we do differently:

- The rule is written down with numbers (criteria 7 to 11), so it can be tested and does not depend on remembering how Inkscape feels. I could not check Inkscape's source from here; the formula is ours and is chosen to give the same result as Inkscape for a drag in the middle of a segment (the curve passes through the grabbed point). The tester checks the formula, not Inkscape.
- **Node types are always honoured** (criterion 12): a Symmetric or Asymmetric node keeps its tangent when the handle next to the bend moves, by the same rule as dragging that handle by hand (`0002`, `0006`).
- The result is shown as the blue new shape over the black old one while the drag runs (`0009-unified-object-editing`), Escape cancels, and the release is one commit.
- It starts only after the pointer has moved more than 3 px, so a click still selects the segment and a double-click still inserts a node (`0002` criteria 12 and 14).

**Where it lives.** In the Node tool, on paths, as a new meaning of dragging a segment. It is not a new tool, a new button or a new shortcut.

## Acceptance criteria

Distances are in document millimetres unless a criterion says "px", which is screen pixels. The document's Y axis points down. A segment runs from node A to node B in the path's stored order, with control points P0 (A's position), P1 (P0 plus A's outgoing handle), P2 (P3 plus B's incoming handle) and P3 (B's position). The closing segment of a closed path runs from the last node to the first.

### Starting a bend

1. Given the Node tool and a path, when the maker presses the primary button at a point that is within the segment tolerance of one of its segments and is not on a node or a handle (the existing hit order: the nearer of node and handle first, a handle winning an exact tie, then the segment within 4 px), then that segment becomes the selected segment, exactly as `0002` criterion 14 says, and the press point and the segment's grab parameter t0 (the parameter of the curve point nearest to the press point) are remembered. Nothing else changes.
2. Given the press of criterion 1, when the button is released and the pointer has never been more than 3 px from the press point, then it was a click: the segment stays selected, nothing is bent and no commit is made. Given two presses on a segment in quick succession with the pointer 2 px apart, then a node is inserted as `0002` criterion 12 says and no bend happens.
3. Given the press of criterion 1, when the pointer has moved more than 3 px from the press point with the button down, then the bend starts. From then on, every pointer move uses the displacement d from the press point (not from the point where the 3 px were passed), as in `0017` criterion 36.
4. Given the hit tolerance, then the press of criterion 1 uses the **Node tool's segment tolerance of 4 px**, not the Select tool's 8 px object tolerance (`0014-advanced-selection` criterion 2; `docs/design-system.md`, "Segment hit-test tolerance"). Test at zoom 100 % and at 800 %: a press 3.5 px from a segment, with the pointer then moved 20 px, bends it; a press 4.5 px away does not (it hits nothing, and the selection clears as it does today).
5. Given a press within 16 px of a node or handle of the same path, then the node or handle is hit and the existing node or handle drag runs, not a bend (the order of criterion 1 is the existing one). Test: on a straight 200 mm segment at 100 % zoom, a press 10 px from the end node on the segment moves the node. The consequence: a segment shorter than about 32 px on screen lies entirely inside the 16 px radii of its two nodes, so it cannot be hovered or bent at that zoom; the maker zooms in.
6. Given the kinds of object, then a bend can start on any segment of an open or closed path the Node tool lets the maker click today, including the closing segment of a closed path. It cannot start on a **compound path** (`0016-boolean-operations` criterion 38: its outlines carry no nodes in the Node tool) and not on a **primitive** (rectangle, ellipse, polygon, star), which has no segments in the Node tool; the maker uses "Object to path" first. A press on those does what it does today. A segment of zero length (both nodes at one point, no handles) has no grab point and cannot be bent.
6a. Given a closed path of two nodes (it has two segments, A to B and the closing segment B to A), when the maker presses on the closing segment, then it becomes the selected segment and can be bent, like the first one. Today the closing segment of such a path cannot be clicked at all. This is a bug fix inside this slice; it also makes "Make line" and "Make curve" (`0002` criterion 14) work on that segment. Test: a closed path of two nodes with both segments curved; a click on each of them selects that segment and not the other.

### The shape rule

7. Given a segment whose outgoing handle at A and incoming handle at B both have zero length (within 1e-9 mm), then it is a straight line and is treated as the cubic with P1 = P0 + (P3 - P0) / 3 and P2 = P0 + 2 (P3 - P0) / 3 before the rule of criterion 8 is applied. This cubic has the same shape as the line. Any other segment is used with its stored P1 and P2.
8. Given the displacement d (criterion 3, in mm), the grab parameter t0 and t = clamp(t0, 1/6, 5/6), then with k = 1 / (3 t (1 - t)) the new control points are P1' = P1 + k d and P2' = P2 + k d. P0 and P3 do not change, so no node moves. The new handles are A's outgoing handle P1' - P0 and B's incoming handle P2' - P3. Test: a straight segment from (0, 0) to (90, 0), pressed at (45, 0) and dragged to (45, -30): t0 = 0.5, k = 4/3, A's outgoing handle is (30, -40), B's incoming handle is (-30, -40), and the curve passes through (45, -30).
9. Given t0 between 1/6 and 5/6, then the curve point at t0 after the bend lies at the original curve point at t0 plus d, within 1e-6 mm. That is, the curve passes through the grabbed point carried by the pointer. Test: a curve with P0 = (0, 0), P1 = (20, 40), P2 = (70, 40), P3 = (90, 0), grab at t0 = 0.3, d = (5, -12).
10. Given t0 below 1/6 or above 5/6 (a grab near a node), then t in criterion 8 is the clamped value, so the handles move at most 2.4 times d, and the curve point at t0 moves by the fraction 3 t0 (1 - t0) / (3 t (1 - t)) of d. Test: t0 = 0.1 gives the fraction 0.648. This is a limit by design: the handles never run away when the maker grabs next to a node.
11. Given a segment that is already a curve, then the same rule applies to its stored P1 and P2 (criterion 8 adds k d to each); the curve is reshaped, not replaced.
12. Given that a handle of a node changes, then the node's other handle follows the rule that applies when the maker drags the first handle by hand: a **Corner** node's other handle does not change; a **Symmetric** node's other handle becomes the mirror (same length, opposite direction); an **Asymmetric** node's other handle keeps its length and turns to the opposite direction of the moved handle (`0002` criterion 9, `0006` criterion 3). If the moved handle's new length is zero, the other handle is left as that rule leaves it for a handle dragged to zero. The segment on the other side of such a node changes shape with it. Test: nodes A (0, 0) Corner, B (90, 0) Symmetric, C (180, 0) Corner, all handles zero; bend AB as in criterion 8: B's incoming handle is (-30, -40), B's outgoing handle becomes (30, 40), and BC is now a curve; with B Corner, B's outgoing handle stays (0, 0) and BC stays a line. The end node of an open path has no segment on its other side, but the hidden handle on that side is still written by the node-type rule, as a hand drag of the shown handle would write it (it stays invisible). A closed path of two nodes has two segments; the rules apply on both ends.
13. Given Shift held, then d is limited to the axis on which the pointer is farther from the press point: horizontal if |dx| > |dy|, vertical if |dy| > |dx|, and on an exact tie the Select tool's rule for a move applies (the previous axis is kept, horizontal when there is none; `0010-edit-interaction-polish`). Shift is read live on every pointer move and, where the platform delivers the key event, on a key change with the pointer at rest; pressing or releasing it mid-drag re-evaluates d at the next frame without a jump of anything else. Platform limit (`0014` plan, "Known limits"): on WebKitGTK the page never receives a Shift key-up, so a Shift release takes effect at the next pointer move; the tester checks the pointer-move route there. While the limit is engaged the Lock badge of `docs/design-system.md` (row "Modifier badge") is shown, switching with the axis, and no axis guide line is drawn. Shift at the press has no effect on a segment.
14. Given the pointer leaves the canvas or the window during the bend, then the pointer stays captured, the bend follows it, and a release outside the canvas commits as a release inside.

### Feedback

15. Given the Node tool, no button down, and the pointer within 4 px of a segment (and not within the node and handle radii), then that segment is drawn with a 4 px band in `--segment-hover` (`--accent` at 50 %) over its whole length between its two nodes, drawn on top of the stroke and under the nodes and handles, constant screen width at every zoom. The band follows what a press would bend: it is absent within the node and handle radii (16 px), for a compound path, a primitive and a zero-length segment, during any drag, and when the pointer leaves the tolerance. A segment that is already selected keeps the selected look (2 px `--accent`) and gets no band. Why 4 px and not the 2 px `--accent-hover` first proposed: `--accent-hover` over a thin black stroke is not visible (UX notes, section 2). At build the flanks are measured at 1x on `--canvas-bg`, on `--pasteboard-bg` and over a filled shape; they must reach 1.5:1, else the opacity is raised (up to 65 %), not the width. The cursor stays the standard arrow (`0002` UX notes).
16. Given a bend in progress, then the old geometry stays drawn in black exactly as committed (its own colour, stroke and fill, not dimmed), and every segment whose shape the bend changes now (the dragged segment, and a neighbouring segment changed by criterion 12) is drawn over it as the "Live preview outline" of the design system: a 1.5 px hollow `--preview-new` line, with the white casing over filled artwork, on every frame. A filled path shows no fill preview. The 2 px selected-segment overlay of the dragged segment is not drawn while the bend runs; it returns on release and on cancel, on the shape that stands then. The two end nodes of the dragged segment show their handle lines and endpoints (both sides, because criterion 12 can move the far side) at their live positions every frame, in the idle handle look, and lose them when the bend ends unless the node is selected (`0002` criterion 7); a handle of length zero is not drawn; the old handle positions are not drawn. The live readout chip shows the displacement d as "Δ x, y mm" (X right, Y down, one decimal, real minus sign; a locked axis reads "0.0"), and with Shift the Lock badge of criterion 13 is shown. Draw order, bottom to top: committed artwork, blue preview, handle lines, node glyphs, handle endpoints, readout and badge.
17. Given a bend in progress, then the preview updates once per animation frame and follows the pointer at 50 frames per second or better on a path of 5000 nodes in the desktop build, measured with scripted pointer moves. The work per move does not grow with the path's node count.

### Commit and cancel

18. Given the release of a bend, then exactly one commit is made, stored as `bend_segment`, that sets the handle values of the anchors named in criteria 8 and 12 and nothing else: no node position, no kind, no style, no other object changes. It is atomic (all handles or none, `0015` criterion 19). Afterwards the bent segment is still the selected segment and no node is selected. Given the new shape is a curve, then the Node bar offers "Make line" for it and not "Make curve" (`0002` criterion 14).
19. Given the pointer is released with a net displacement d below 1e-9 mm (dragged away and back to the press point, or Shift made d zero), then nothing is written and no commit is made; a line stays a line with no stored handles.
20. Given a bend in progress, when Escape is pressed or the system cancels the pointer, then the preview, the readout and the Lock badge disappear, the path is as committed, the segment stays selected (its 2 px overlay is drawn again on the unchanged shape), and the release writes nothing. One Escape ends the drag and nothing more (`0010` criterion 45); a second Escape clears the selection.
21. Given the Select, Pen, Rectangle, Ellipse and Polygon/Star tools, then they behave as before: a drag on a path's outline in the Select tool still moves the object; the Pen still starts a new path; nothing bends.
22. Given this slice, then no undo exists (`0020-undo-redo` is not built). The bend is one commit so that undo can take it later.
23. Given the tool rail, then the tooltip of the Node tool reads "Node tool (N)" with the second line "Drag a segment to bend it. Shift: one axis". There is no hint chip over a segment.

## Out of scope

- **Bending a compound path or a primitive** (criterion 6). Compound-path node editing is `0016` Question 7; primitives are converted with "Object to path".
- **Bending several segments at once** (a selection of two segments dragged together). Only one segment is ever selected.
- **Alt and Ctrl variants.** The Alt "nearer handle only" variant is dropped (drag that handle instead; Alt is unreliable on Linux desktops). Ctrl "keep handle directions" is not built; if wanted, it ships later with its own badge.
- **Keyboard bending** (Arrow bends by 1 mm, Shift+Arrow by 10 mm): proposed for the nudge story, not built here. **Typed numbers** for the bend, **snapping** to the grid, nodes or guides, **a pressure-driven bend** (`0032-pen-tablet-input`).
- **Keeping the curve's shape when it is changed** by a bend (curve fitting), **Inkscape's "slide the grab point along the curve"**.
- **Selecting the end nodes after a bend** (Question 2).
- **A bend cursor** (Question 3).
- **The blue and black preview for node and handle drags** of the Node tool: they keep their live redraw (`0009` Question 6). Only the bend uses it.
- **Undo and redo** (`0020-undo-redo`).
- **A hint chip over a segment** (UX Question 6: none) and a **bend cursor** (Question 3).

## Proposals (not in the criteria until the customer accepts them)

- **Ctrl: keep handle directions.** With Ctrl held, the handles only change length, never direction. Useful to keep a smooth curve's tangents. Not built; it would need its own on-screen badge. (The Alt variant "move only the nearer handle" was dropped on 2026-10-10: the same result exists and is exact by dragging that handle.)
- **Keyboard bending.** With a segment selected, Arrow bends it by 1 mm and Shift+Arrow by 10 mm with the rule of criteria 8 to 12, one commit per key press. Belongs with the nudge story.
- **A bend cursor** (an arrow with a small curve) while the pointer is over a bendable segment. `0002` fixed the Node tool's cursor to the arrow; the hover highlight of criterion 15 already says "this is a segment".

## Open questions

Each has a default; nothing blocks.

1. **Shape rule (criteria 8 to 10).** *A (default):* both handles move by the same vector k d with t clamped to [1/6, 5/6]. The curve follows the pointer in the middle two thirds, and the effect fades near a node. *B:* a rule closer to Inkscape if you notice a difference when you try it (for example the nearer handle alone at the ends); I cannot verify Inkscape's exact rule. *C:* (dropped: the Alt variant; drag the handle instead). Recommendation: A, then adjust after you have tried it; the rule is one function and one test table.
2. **Select the end nodes after a bend.** *A (default):* the segment stays selected and no node is, as before. *B:* both end nodes become selected, which shows their handles and lets the maker refine them at once (Inkscape selects both end nodes of a clicked segment). B changes what a click on a segment does (`0002` criterion 14), so it is a separate decision.
3. **Cursor.** Default: the arrow (the Node tool never changes its cursor). Option: a bend cursor over a segment.
4. **Ctrl.** Default: not built (Proposals); Alt is dropped. Say if you want Ctrl "keep handle directions".
5. **Blue and black in the Node tool.** Default: only the bend shows the old shape in black under the new one in blue; node and handle drags keep redrawing the object. Option: use blue and black for those drags too, so the Node tool is consistent (`0009` Question 6).

6 to 8 are the UX questions at the end of the UX notes (hint chip: none; keyboard bending: with the nudge story; Ctrl: see 4).

Decided by the product owner (change if you disagree): the dead zone of more than 3 px; the displacement is measured from the press point and the curve point is carried by it (no jump to the pointer); Shift limits the drag to one axis, as for a move; the hover band in `--segment-hover` (4 px, the UX engineer's correction of the first 2 px `--accent-hover`); the 4 px tolerance (not 8 px); a bent line becomes a curve and stays one even when dragged to nearly straight; compound paths and primitives excluded.

## UX notes

By `ux-engineer`, 2026-10-10. Numbers and tokens are in `docs/design-system.md` (rows "Segment hover highlight", "Segment bend preview", "Bend readout and badge", token `--segment-hover`, and the Node tool tooltip). Where a note and a criterion differ, the criterion stands. The changes listed at the end ("Criteria changes requested") were applied to criteria 13, 15, 16, 20 and 23 on 2026-10-10; the Alt "nearer handle" variant is dropped.

### 1. What the maker sees, step by step

| Moment | Canvas | Cursor |
|---|---|---|
| Pointer rests within 4 px of a bendable segment (no button down) | The segment's whole length between its two nodes gets the hover band (below). Nothing else changes. | Arrow |
| Press on it, pointer not yet 3 px away | The segment is selected: the 2 px `--accent` overlay of `0002`. This is the same as a click today. | Arrow |
| Pointer 3 px or more away, button down | The bend runs: the selected overlay is replaced by the blue preview (section 3), the end-node handles appear, the readout follows the pointer. | Arrow |
| Release | One commit. The blue preview is gone, the object shows the new shape in its own style, the segment is selected again (2 px overlay on the new curve), the readout and handles of the bend are gone. | Arrow |
| Escape, or the system cancels the pointer | Preview, readout and badge disappear, the segment is selected again on the unchanged shape. | Arrow |

### 2. Hover highlight

- **Look.** A 4 px wide band centred on the segment, colour `--segment-hover` (new token: `--accent` at 50 %), drawn on top of the path's stroke and under the nodes, handles and every other overlay, constant screen width at every zoom, round caps ending at the two nodes' centres.
- **Why not the 2 px `--accent-hover` of criterion 15.** `--accent-hover` is `--accent` at 20 %. Laid over a black stroke it gives about RGB 9, 22, 48 against 0, 0, 0, and over `--canvas-bg` the flanks measure about 1.25:1: on a thin black line the hover would not be visible at all. The skew guide had the same defect and was fixed the same way (a stronger value, a new token, `docs/design-system.md` "Transform skew fixed-line guide"). At 50 % and 4 px the stroke shows as a dark blue core with a pale blue band on both sides; the band measures about 2.0:1 against `--canvas-bg` and about 1.55:1 against `--pasteboard-bg`, and it is clearly wider and paler than the selected overlay (2 px, solid `--accent`, 3.9:1), so hover and selected cannot be mistaken for each other. **Measure at build** from the GL buffer at 1x on `--canvas-bg`, on `--pasteboard-bg` and over a filled shape; if the flanks fall below 1.5:1 anywhere, raise the opacity (up to 65 %), not the width.
- **It shows what a press would bend, nothing more.** The same test as the press (criterion 1): absent within the node and handle radii (16 px) of the same path, absent for a compound path, a primitive and a zero-length segment, absent while any drag runs, and absent on the selected segment (it keeps the selected look, criterion 15). A segment shorter than about 32 px on screen lies entirely inside its two nodes' 16 px radii and can show no hover and cannot be bent; the maker zooms in. This is the cost of criterion 5 (nodes win) and is the right trade; no special case.
- **No highlight without a pointer.** Touch has no hover; a pen shows it while hovering (`0032`). The press still selects and the drag still bends, so nothing depends on seeing the hover.

### 3. During the bend

Draw order, bottom to top: the artwork exactly as committed ("black old", its own colour, stroke and fill, not dimmed) / the blue preview / handle lines / node glyphs / handle endpoints / DOM (readout, badge).

- **Blue preview.** Every segment whose shape the bend changes now (the dragged one, and a neighbour changed through a Symmetric or Asymmetric node, criterion 12) as the row "Live preview outline": 1.5 px hollow `--preview-new`, no fill, with the white casing over filled artwork. One line per changed segment, each from node to node. The nodes do not move, so the blue line starts and ends exactly on the black one.
- **The selected-segment overlay is not drawn while the bend runs.** Two blue lines of different weight on the old and the new shape would not say which is which; the 2 px overlay returns on release or cancel, on the shape that stands then. (Criterion 16 as written does not say this: change requested.)
- **Handles.** The handle lines and endpoints of the two end nodes of the dragged segment, on both sides of each node, in their live positions every frame: handle line 1 px `--accent`, endpoint 12 px circle, white fill, `--accent` ring (the idle look of `0002`). They are idle on purpose: the fill `--accent` means "being dragged by the pointer", and the pointer drags the curve, not a handle. A handle of length zero is not drawn (a Corner node's untouched far side stays invisible). Handles of nodes further away are not drawn. The old handle positions are not drawn: only the new state has handles.
- **Readout.** The existing live readout chip (12 px up and right of the pointer, `--toolbar-bg`, 12 px text) shows the pointer's displacement d as the move readout does: "Δ 12.5, −3.0 mm" (X right, Y down, one decimal, real minus sign; after Shift the locked axis reads "0.0"). It is the one number the maker can check; it is not a curve parameter, and the readout says nothing about handle lengths (they are the result, not the input).
- **Shift badge.** While Shift limits d to one axis the Lock badge of the row "Modifier badge" shows (16 px, 16 px left of and below the pointer, horizontal double arrow for a horizontal limit, vertical for a vertical one, switching in the frame the axis does). **No axis guide lines** (the move has two): the blue curve moving along one axis, the badge and the "0.0" are enough, and a bend is a smaller gesture where full-width guides would be louder than the thing being edited.
- **Cursor** stays the arrow, hover and drag, as for a node or handle drag in the Node tool. No bend cursor (Question 3, default stays): the hover band already says "this is a segment you can take", and a changing cursor would need a custom image per platform for one hint.
- **Motion.** None. Nothing fades or eases; `prefers-reduced-motion` needs nothing.

### 4. Modifiers

| Key | In the bend | Reason |
|---|---|---|
| Shift | Limits d to one axis (criterion 13), the Lock badge shows. At the press it does nothing on a segment (no add to selection). | The meaning of Shift for a move (`0010`), the same key, the same badge. |
| Ctrl | Free, not built. | Proposal "keep handle directions" is consistent with Inkscape's Ctrl on a handle (constrain the angle). If the customer asks for it: Ctrl, with its own badge, because a held key with no on-screen sign is the thing the Lock badge exists to avoid. |
| Alt | Not used. | KDE, xfwm and older GNOME take Alt+drag for the window, and Firefox and Windows react to a lone Alt release (`0014` plan, "Known limits"); the marquee already carries that risk. A bend that fails on the customer's own desktop is worse than a missing variant. |
| Alt proposal "nearer handle only" | Recommend dropping it. | The same result already exists and is exact: drag that handle. |

**Known limit (`0014` plan.md).** On the customer's Linux and WebKitGTK setup the page never receives a Shift key-up. The modifier state is corrected at the next pointer event, so with a stationary pointer the lock and its badge stay until the pointer moves. Criterion 13's "on a key change with the pointer at rest" holds for the press and for platforms that deliver the event; the tester checks the pointer-move route on this setup.

### 5. Discoverability

- **The rail tooltip of the Node tool** gets a second line, as Rectangle and Ellipse have: "Node tool (N)" / "Drag a segment to bend it. Shift: one axis". That is the one place the gesture is named.
- **No hint chip.** The 600 ms hint chip belongs to handles because each handle has several meanings (Shift, Ctrl, double-click). A segment has one meaning, and a rest over a segment is the normal state of someone aiming at a node; a chip there would fire all the time and teach nothing the hover band does not. Option for Question 6: a chip "Drag: bend. Double-click: add a node." after a 600 ms rest on the highlighted segment. Default: none.
- The Node bar and the hint chips need no change (as the facts in the spec said). After the bend, "Make line" appears (criterion 18); that is the bar's existing behaviour.

### 6. Accessibility and states

- **Keyboard.** Bending is a pointer gesture, like every canvas edit of the Node tool today (nodes, handles and the marquee have no keyboard route either; nudging is "not built" in the keyboard table). A drag is essential to this operation and its direct alternative is the existing one: select the segment, then move its handles by hand. The keyboard route belongs to the nudge story: with a segment selected, Arrow bends it by 1 mm and Shift+Arrow by 10 mm with the same rule (criteria 8 to 12 with d from the key), one commit per key press. Proposal, not built (Question 7).
- **Targets.** The segment tolerance stays 4 px (an 8 px wide target; criterion 4); node and handle radii are 16 px (32 px targets). The 4 px band is a geometry target, not a control: the 24 px minimum target size does not apply to canvas geometry, and 8 px is what keeps neighbouring segments pickable.
- **Colour is not the only cue.** Hover is a wider, paler band than the selected overlay, selected is a thinner solid line, the preview is a hollow line over the black one, and the bend always shows the number in the readout.
- **Nothing silent that should speak.** A press on a segment that cannot be bent (compound path, primitive, zero-length segment) does what it does today: no hover, no error. The compound path's pill ("Nodes of compound paths cannot be edited yet.") and the primitive's hint chip already say why.
- **Touch and pen.** Pointer capture (criterion 14) keeps the drag when the pen or finger leaves the canvas; the 3 px threshold is small for a finger, which the touch story (`0032`, later) may enlarge; not decided here.
- **Large documents.** Hover and preview touch the hovered segment and its two neighbours only (criterion 17): the cost per frame must not depend on the path's node count, also for the hover test (spatial lookup, not a scan of the whole path on every pointer move).
- **Zoom.** Band, preview and handles are screen-constant. At high zoom a segment can be longer than the screen: the band is clipped at the viewport edge, not drawn off screen to its node.
- **Undo.** None in this slice; the single commit is what `0020` will take.

### 7. Criteria changes requested (by number)

- **13:** add the platform limit of section 4 ("where the platform delivers the key event; on WebKitGTK a Shift release takes effect at the next pointer move"). Add: while the lock is engaged the Lock badge of `docs/design-system.md` is shown, and no axis guide is drawn.
- **15:** replace "a 2 px line in `--accent-hover`" by "a 4 px band in `--segment-hover` (`--accent` at 50 %), drawn on top of the stroke and under the nodes and handles"; add that it follows what a press would bend (absent within the node and handle radii, for a zero-length segment, during any drag). Keep the rest.
- **16:** add (a) the selected-segment overlay of the dragged segment is not drawn while the bend runs and returns on release and on cancel; (b) the white casing of the row "Live preview outline" applies over filled artwork; (c) the handles of the end nodes are drawn at their live positions every frame, a handle of length zero is not drawn; (d) the readout "Δ x, y mm" and, with Shift, the Lock badge are shown (new sub-criterion 16a if you prefer a separate test).
- **20:** add that the readout and badge disappear too.
- **New 23:** Given the rail, then the tooltip of the Node tool reads "Node tool (N)" with the second line "Drag a segment to bend it. Shift: one axis".

### 8. Open design questions (defaults taken)

6. **Hint chip over a segment.** Default: none (section 5). Option: the chip after a 600 ms rest.
7. **Keyboard bending.** Default: not in this slice; proposed with the nudge story (section 6).
8. **Ctrl "keep handle directions".** Default: not built (section 4); if wanted, it ships with its own badge.

## Links

Requirements: R-EDIT-020, R-EDIT-001 (`docs/requirements.md`)
Builds on: `specs/0002-path-node-editing/` (segment selection, make line and curve, node and handle model), `specs/0006-path-merge-split-and-node-types/` (Corner, Symmetric, Asymmetric), `specs/0014-advanced-selection/` (criterion 2: the Node tool's 4 px segment tolerance), `specs/0009-unified-object-editing/` (blue and black preview), `specs/0010-edit-interaction-polish/` (Escape, Shift)
Related: `specs/0016-boolean-operations/` (compound paths), `specs/0020` (undo, reserved)
ADRs: `adrs.md` (architect, 2026-10-10: no new ADR; the architect's rewordings of criteria 1, 3, 12 and 13 and the two-node closed path fix, criterion 6a, are applied)
PR: TBD
