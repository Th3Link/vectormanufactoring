# Boolean operations

Status: Ready
Priority: Must
Origin: Customer (R-EDIT-003, in the confirmed MVP cut). Customer decisions of 2026-10-09 (final): compound-path results are approved (Question 1, A, including the document-model and file-format change); Exclusion and Reverse difference are in (Question 2, yes), so there are five operations; all boolean operations are a command section in the left tool rail. Everything marked **Proposal** (only the preview, below) is the product owner's idea and is not accepted until the customer says so.

## User value

As a maker I want to combine, cut and overlap shapes (union, difference, intersection, exclusion, reverse difference) so that I get one clean closed outline for my laser, for example a plate with mounting holes, a bracket from two rectangles, or a keyhole from a circle and a slot, without redrawing it by hand.

**What we match from Inkscape and LightBurn, and what we do differently:**

- **Same direction rule as Inkscape: Difference is the bottom object minus the objects above it, and the result keeps the bottom object's look.** Inkscape's manual says the stacking order decides which object the operation applies to. LightBurn uses the order in which you clicked and tells you to undo and run it again to flip it. We use the stacking order only, so the same drawing always gives the same result, however the objects were selected.
- **Any number of objects in one step.** Inkscape (through 1.2) accepts exactly two paths for Difference, Exclusion, Division and Cut Path; LightBurn's Boolean tools take exactly two shapes and its Weld takes more. Here all five operations (Union, Difference, Intersection, Exclusion, Reverse difference) take two or more.
- **Closed paths only, as R-EDIT-003 says and as LightBurn does.** Inkscape closes an open path silently, which turns a "U" into a "D" without a word. We refuse and say which object is open.
- **An empty result is refused, not applied.** Inkscape removes the operands and leaves nothing. There is no undo yet (`undo-redo`, 0020, is not started), so deleting the operands for an empty result would be unrecoverable.
- **What is worse than Inkscape, said plainly:** the result consists of straight segments, not curves. A union of two circles comes back as a polygon with a few dozen nodes per circle (ADR 0003 §3; deviation from the true curve at most 0.01 mm). That is exact enough to cut and is the same polygon every machine output uses, but it is harder to edit by node than Inkscape's curve-exact result. Refitting curves afterwards is a follow-up (see Out of scope), not part of this slice.
- **What is better than LightBurn, as a Proposal:** a preview of the result before it is applied (Proposals, below). LightBurn has this as its Boolean Assistant.

## Operations

| Operation | Result region | In this slice |
|---|---|---|
| Union | Everything covered by at least one operand | MVP, Customer |
| Difference | The bottom-most operand minus everything covered by the other operands | MVP, Customer |
| Intersection | Only what every operand covers | MVP, Customer |
| Exclusion | What an odd number of operands cover (for two operands: covered by one, not both) | In, Customer (decided 2026-10-09) |
| Reverse difference | The top-most operand minus everything covered by the other operands; the same as Difference with the top-most object first | In, Customer (decided 2026-10-09) |
| Division, Cut Path, Combine, Break Apart | not in this slice | Out of scope |

Reverse difference exists because the product has no "bring to front" or "send to back" command yet. Without it, the only way to subtract the lower shape from the upper one is to redraw them in the other order.

## Words used below

- **Operand:** a selected object that takes part in the operation.
- **Bottom-most / top-most:** by stacking order (z-order, ADR 0002 §5), never by the order in which the objects were selected.
- **Region of an operand:** the area its outline encloses, filled with the nonzero rule, which is the rule the canvas uses to paint a fill (`0007-stroke-and-fill-styling` criterion 14).
- **Base operand:** the operand the result is made from. The bottom-most operand, except for Reverse difference, where it is the top-most.
- **Compound path:** one object made of two or more closed outlines, filled with the nonzero rule so that an outline wound against its surrounding outline is a hole. This is what a result with a hole or with separate pieces needs. The current path object holds exactly one outline (see "Compound paths" below).
- **Kernel tolerance:** 0.01 mm. Curves are flattened to straight segments that stay within 0.01 mm of the true curve. It is fixed, not a setting and not tied to zoom (ADR 0003 §7: display and manufacturing tolerances must not be shared).

## Acceptance criteria

### Entry points

1. Given any state of the app, then the left tool rail shows a Boolean section under the tools, behind a divider, with five buttons in this order: Union, Difference, Intersection, Exclusion, Reverse difference. Nothing in the rail appears, disappears or moves with the selection or the tool. Given the Select tool is active and two or more objects are selected (any mix of path, rectangle, ellipse, polygon, star and compound path, closed or open), then the buttons are enabled at full contrast. Given any other state (none or one object selected, or a tool other than Select active, in which case the object selection is not drawn or is cleared), then the buttons are dimmed (40 % opacity), `aria-disabled="true"`, still focusable, with the tooltip note "Select two or more closed objects." (with a tool other than Select active, followed by the hint "Use the Select tool."); activating one does nothing and writes nothing. The state is one pure function in `curvyo-ui-core`, `boolean_availability`, with the values NeedsTwo (dimmed), OpenPaths (enabled; the tooltip names the open paths and a click shows the refusal of criterion 15) and Ready; the frontend only displays it and decides nothing.
1a. Given any boolean button is pressed or activated, then the active tool does not change, no boolean button has `aria-pressed`, and no boolean button is shown as pressed or active (only the transient `:active` ground while the pointer is down or the operation is busy); the active-tool look stays on exactly one tool button, before, during and after.
2. Given the Boolean section, then it is one Tab stop with roving focus (Up, Down, Home and End move the focus without activating, no wrap; Space or Enter activates; dimmed buttons stay in the set), and every button has an accessible name equal to its operation name and a tooltip, opening to the right of the button, of three lines: the name, the rule (for Difference: "The lowest selected object minus the others.") and a note, with no shortcut text. The note reads "Replaces the selection. No undo yet." in the normal case, "Needs closed paths: 2 of 3 selected are open." when an open path is selected, and "Select two or more closed objects." as in criterion 1. Given an open path is selected together with another object, then the buttons stay enabled, and activating one by mouse or by key shows the refusal of criterion 15.
3. Given any state of the app, then the rail section is the only way to start a boolean operation: no keyboard shortcut, no menu item and no context-menu entry (no shortcuts for now; default of Question 3). Object to path has no shortcut either, for the same reason: there is no undo yet.

### What an operand contributes

4. Given any operand, then its region is the area enclosed by its outline under the nonzero rule, exactly the area the canvas paints when Fill is Solid, whether or not Fill is on.
5. Given operands whose paint differs (Fill None, Fill Solid at opacity 0 %, Stroke off, Stroke 10 mm wide), then the result is the same as for the same outlines with any other paint. Test: a stroke-only square and a filled square unite to the same outline as two filled squares.
6. Given a rectangle (with or without corner radii, rotated or not), ellipse, polygon or star as operand, then its region is the shape as drawn, with no prior Object to path needed. Test: a rounded rectangle minus a circle that crosses one of its rounded corners.
7. Given a self-intersecting path as operand (a five-point star drawn as one closed path of five nodes that crosses itself), then its region includes its centre, exactly as the canvas paints it. Test: the union of that star and a square far away has an area equal to the star's painted area plus the square's.
8. Given a compound path (the result of an earlier operation) as operand, then its holes stay holes. Test: a ring (disc of radius 20 mm minus a concentric disc of radius 10 mm) united with a concentric disc of radius 5 mm gives three outlines (outer 20, hole 10, island 5) and an area of π(400 − 100 + 25) mm², within 0.01 mm times the total outline length of the exact result.
9. Given the same objects at the same stacking positions, then the result is the same however the selection was made (click order, marquee, Shift-click). Test: select A then B, and B then A.

### The operations

10. Given two or more operands, when the maker activates Union, then the result region is the union of all operand regions. Test: two squares of side 20 mm, the second offset by (10, 10) mm, give one outline of 8 nodes and area 700 mm².
11. Given two or more operands, when the maker activates Difference, then the result region is the bottom-most operand's region minus the union of all other operands' regions. Test: three operands A (bottom), B, C give A − (B ∪ C).
12. Given two or more operands, when the maker activates Intersection, then the result region is the region covered by all operands. Test: three overlapping discs give the lens common to all three.
13. Given two or more operands, when the maker activates Exclusion, then the result region is covered by an odd number of operands. Test: three overlapping discs give the parts covered once or three times, not the parts covered twice.
13a. Given two or more operands, when the maker activates Reverse difference, then the result region is the top-most operand's region minus the union of the others, which is the same result as Difference with the stacking order read from the top. Test: for operands A (bottom) and B (top), Reverse difference gives B − A, and its result takes B's style and place (criteria 21, 22).
14. Given random polygon pairs A and B with straight edges in the test suite (at least 200 pairs, fixed seed, including pairs with holes and self-intersections), then the areas obey area(A ∪ B) + area(A ∩ B) = area(A) + area(B) and area(A − B) + area(A ∩ B) = area(A), each within the bound `1e-6 * (area(A) + area(B)) + 0.001 mm * (perimeter(A) + perimeter(B))`, where the perimeter is the total length of all outlines of the operand's region. The second term is one grid pitch (criterion 39) of boundary movement along the operands' edges: snapping to the grid and the node removal of criterion 24 move the boundary by up to that much, so a pure 1e-6 bound cannot hold on the 0.001 mm grid. The same bound applies to the identity for Exclusion, area(A ⊕ B) = area(A) + area(B) − 2·area(A ∩ B). Pairs of coarse-coordinate shapes without features smaller than the grid pitch hold the identities at 1e-6 relative alone (the tests assert both).

### Refusals: nothing changes

"Nothing changes" means: the same objects with the same ids, order, geometry and styles, the same selection, the same active tool, no commit written to the document.

15. Given any operand is an open path, when the maker activates any operation, then the operation is refused and a refusal notice appears beside the rail (a chip over the canvas whose top edge is level with the top of the Union button and whose left edge is 12 px right of the rail card, not under a Select-bar group) for 8 seconds or until the next press, key, selection change or tool change. It reads "<Operation> needs closed paths. 1 of 3 selected objects is open. Nothing was changed." (plural: "2 of 3 selected objects are open."). While the notice is shown, each offending operand is drawn with a hollow 2 px red outline (`--field-invalid`, with the white casing) over the canvas; the outline is not stored and does not change the selection.
16. Given any operand encloses no area (all nodes on one line, or all nodes at one point, for example a closed path of two nodes), then the operation is refused with the notice "<Operation> needs shapes that enclose an area. 1 of 3 selected objects has no area. Nothing was changed.", shown and outlined as in criterion 15.
17. Given the result region is empty, then the operation is refused with one of these notices: Intersection "Intersection is empty: the selected objects share no area."; Difference "Difference is empty: the lowest object is covered completely."; Reverse difference "Reverse difference is empty: the top object is covered completely."; Exclusion "Exclusion is empty: no area is covered an odd number of times."; each followed by "Nothing was changed." Test cases: Intersection of two discs that do not touch; Intersection of two squares that share only an edge or only a corner; Difference where the top object covers the bottom object completely; Exclusion of two identical squares.
18. Given a refusal, then no modal dialog opens, the refusal notice is an alert (`role="alert"`), the buttons stay usable, and a second activation gives the same result. A mouse activation returns focus to the canvas; a keyboard activation leaves it on the button.

### The result

19. Given a successful operation, then all operands are removed and exactly one new object takes their place. Objects that were not selected keep their ids and their order relative to each other.
20. Given the result region has one outline and no holes, then the new object is an ordinary closed path that the Node tool edits like any other path. Given the result has a hole or several separate pieces, then it is one compound path object, not several objects.
21. Given a successful operation, then the new object's complete style (stroke on or off, width, colour, opacity, dash, join, cap; fill on or off, colour, opacity) is a copy of the base operand's style.
22. Given a successful operation, then the new object sits at the base operand's place in the stacking order, directly where that operand was. Objects between the operands that were not selected stay where they are.
22a. Given a successful operation or a refusal, then after a mouse activation keyboard focus is on the canvas, and after a keyboard activation it stays on the pressed button (which is dimmed after a success because one object is selected; focus on a dimmed button is allowed). Nothing in the rail unmounts, so focus is never lost.
23. Given a successful operation, then the new object is the only selected object, the active tool is unchanged (it is the Select tool, because the buttons are only enabled there), and the Properties panel describes it (subject line "Compound path" when the result is one).
24. Given a successful operation, then every node of the result is a Corner node with no handles, no two consecutive nodes are closer than 0.001 mm, and no node lies on the straight line between its neighbours (within 0.001 mm). Test: the union of two squares of side 20 mm that share a full edge has 4 nodes.
24a. Given any operation, then removing nodes (criterion 24) never moves the boundary by more than one grid unit (0.001 mm) from the outline it is applied to: every point of the cleaned outline lies within 0.001 mm of the outline before cleanup, and every point of the outline before cleanup lies within 0.001 mm of the cleaned one. This is the measurable form of criteria 24 and 25 for the cleanup step; it holds for every outline of the result, holes included.
24b. Given an operand polyline with many nearly collinear vertices (test: 100,000 vertices on a circle of radius 10 mm, united with a far-away square), then the cleaned outline keeps its shape: every point of it lies within 0.001 mm of the input polyline (after snapping to the grid) and vice versa, with no cumulative drift however many consecutive vertices are removed, and its area differs from the input polygon's by at most 0.001 mm times the outline length. It completes without panic or hang (no time budget is set for this input size; criterion 45 covers 10,000 nodes).
25. Given an operand with curves, then the result's outline lies within 0.01 mm of the exact boolean result of the operands' curves. Test: the union of two discs of radius 10 mm with centres 5 mm apart; every node of the result lies within 0.01 mm of the true outline, and the straight segments between nodes deviate from it by at most 0.01 mm.
26. Given a disc of radius 10 mm (4 Bézier segments) and a far-away rectangle as operands of a union, then the disc's outline in the result has between 71 and 142 nodes. This is the node budget: the fewest straight segments that keep every chord within 0.01 mm, and at most twice that.
27. Given a successful operation, then the new object's rotation is 0, so its selection box is aligned with the document axes.
28. Given a successful operation, then the document records exactly one commit, whose label names the operation (stored as `boolean_union`, `boolean_difference`, `boolean_intersection`, `boolean_exclusion`, `boolean_reverse_difference`; the undo slice maps them to display text). The operands are never removed without the result being added, and the reverse never happens either, however the operation ends. This is what lets `undo-redo` treat it as one step later.
29. Given a successful operation, then a transient notice of one line names the operation and the counts and ends with "No undo yet.", for example "Union: 3 objects became 1 path. No undo yet." or "Difference: 2 objects became 1 compound path. No undo yet." It disappears within 3 seconds or on the next action, takes no focus and is a `role="status"` live region.
30. Given a result is saved and the project reopened, then the object has the same outlines, node order and style as before closing.

### Compound paths

Today a path object holds exactly one outline (`0002-path-node-editing`, Out of scope: compound paths; ADR 0002 §6 already plans "explicit open/closed subpaths"). A difference with a hole inside, or a union of two separate shapes, cannot be one object without more than one outline per object. This part is the minimum that makes such a result usable. It is a document-model and file-format change, so per `CLAUDE.md` §3 it needed the customer: **approved by the customer on 2026-10-09 (Question 1, A)**. The architect designed it: a path keeps its first outline in the existing keys and stores further outlines in one new optional list, `extra_subpaths`; every anchor of every outline has its own unique id; the fill rule is nonzero over all outlines of the object together. The architect's audit (`adrs.md`, "The audit") lists every place that assumes one outline. Each place needs its own test; the criteria below cover them as follows:

| Audit place | Criteria |
|---|---|
| File reading, writing and validation of outlines (`path_codec` and a new `subpath_codec`) | 30, 37, 37a |
| Rotate, scale, skew of a path snapshot (`rotated`, `scaled`, `sheared`) | 35 |
| Move, Duplicate (anchor renumbering and id checks), rotate commit, resize commit (`resize_path`), skew commit | 35, 36 |
| Point-in-outline test over several outlines (`contains_point`) | 33 |
| Hit-testing, object bounds, oriented box (`ui-core`) | 33, 34 |
| Fill, stroke and dash drawing (`render-core`) | 31, 31a, 32 |
| Node tool, Join, Split | 38, 38a |
| Stroke markers | 38b |

31. Given a compound path with Fill Solid (a disc of radius 20 mm minus a concentric disc of radius 10 mm), then the canvas paints the ring only: a pixel at the centre shows what is behind the object (the canvas colour, or a lower object), not the fill.
31a. Given a compound path with a dash pattern, then the pattern starts afresh at the first node of each outline, so every outline is dashed on its own.
32. Given a compound path with Stroke on, then every outline is stroked, with the object's width, dash, join and cap.
33. Given a compound path, when the maker clicks inside a hole, then this object is not picked (what is behind it is, or nothing). When the maker clicks on the fill between the outlines, or on the stroke of any outline, then this object is picked.
34. Given a compound path, then its selection box is the bounding box of all its outlines.
35. Given a compound path is selected, when the maker moves, resizes, rotates or skews it (by drag or by typed values), then every outline transforms together and holes stay holes. Test: rotate the ring by 37° and scale it to 150 % by 50 %; the point that was the ring's centre is still not painted. The same transforms by typed values, and a move, give the same outlines.
35a. Given a compound path and the switch "Scale stroke width" on, when the maker resizes it, then the stroke width is scaled once for the object, not once per outline.
36. Given a compound path, then Duplicate, Delete, copy-by-Ctrl-move and every style edit work as on any object. Object to path is not offered for it (it is a path already).
36a. Given a compound path is duplicated, then the copy has the same outlines, and every anchor of the copy (in every outline) has a new id that is unique within the document; the original keeps its ids.
37. Given a compound path is saved and the project reopened, then it has the same outlines in the same order and winding. A file saved by this build that contains a compound path and is opened by an earlier build is refused with the "saved by a newer version" message (`format_version` is raised to `main`'s current value plus one at merge; `main` is at 7). A file from an earlier build opens unchanged and is not rewritten on open.
37a. Given a file whose extra outlines are damaged (the list is not a list of outlines, or an anchor is malformed), then it is refused as damaged and does not crash. A compound path with no extra outlines is written as an ordinary path.
38. Given a compound path is the only selected object and the Node tool is active (default of Question 7), then the outline is shown, no node, handle or segment overlay is shown, and the Node bar slot shows a text-only line "Nodes of compound paths cannot be edited yet." Given the maker double-clicks a compound path with the Select tool, then the tool does not change and a hint chip shows the same sentence for 3 seconds. Move, resize, rotate, skew, Delete and style edits are unaffected. Ordinary one-outline paths, including results of criterion 20, are node-editable as before.
38a. Given a compound path is part of a selection in the Node tool, then it contributes no node, and Join and Split are not offered for it and refuse if called; nothing changes.
38b. Given a compound path is selected, then the Markers block of the Properties panel is not shown and no marker is drawn for it. This is the default until `0018-stroke-markers` decides a rule for markers on compound paths (flagged to that spec).

### Precision, robustness and determinism

39. Given the kernel works on a grid of 0.001 mm, then coordinates that round to the same grid point are treated as coincident, and edges 0.002 mm or more apart are treated as separate. (Two values 0.0004 mm apart can round to neighbouring grid points, so the tests use grid-aligned positions.) Test: a square with its right edge at x = 20.0000 mm and a square with its left edge at x = 20.0004 mm unite to one 4-node rectangle; with the second edge at x = 20.0020 mm they unite to a compound path of two outlines.
40. Given each of these fixtures (in `tests/fixtures/`, golden files, taken from real maker files where possible), then each operation completes in under 2 s without panic or hang and either gives a valid result or one of the refusals of criteria 15 to 17: (a) two identical squares, (b) two squares sharing a full edge, (c) two squares sharing only a corner, (d) a square inside a larger square with one shared edge, (e) a contour with a duplicated node, (f) a contour with a zero-length segment, (g) a bow-tie (self-intersecting four-node path), (h) a rectangle 0.005 mm by 100 mm, (i) one compound operand with 5,000 disjoint tiny squares. Expected for (a): Union and Intersection give the same square (4 nodes); Difference and Exclusion are refused as empty. Expected for (c): Union gives one compound path of two outlines; Intersection is refused as empty.
41. Given a valid result, then every outline has at least 3 nodes and an area greater than 0, no outline crosses itself or another outline, and every coordinate is a finite number. Holes lie inside an outline, islands inside holes, and the winding rule is fixed: outer outlines run one way and holes the other (documented in the ADR).
42. Given operands placed up to 100 000 mm from the document origin, then criteria 24 and 25 hold unchanged.
43. Given the same operands run twice, then the result is identical node for node (coordinates, outline order, start node, direction). Given a result computed on Linux, Windows, macOS and in the browser build, then the golden files agree to 1e-6 mm. Reversing an operand's winding and rotating its start node give an identical result for every operation, and Union and Difference are also identical under any reordering of operands (Difference: of the operands above the base). **Accepted limitation:** for Intersection and Exclusion with three or more operands, permuting the operands may change the node order and the area by up to the grid tolerance (bounded by 0.001 mm times the total outline length; observed up to about 0.01 mm² on random test shapes). The operations are folded pairwise and grid rounding differs with the fold order. This is accepted because the product defines the operand order by stacking order, never by click order (criteria 9 and 22), so the same drawing always gives the same result.

### Performance

Measured on the CI runner, release build, native; the browser build may take up to twice as long.

44. Given two operands of 1,000 nodes each (curved segments, partly overlapping), then each of Union, Difference and Intersection returns within 100 ms (kernel call, without the document update).
45. Given two operands of 10,000 nodes each, then each returns within 2 s.
46. Given 1,000 rectangle operands (4 nodes each, in a grid, neighbours overlapping), then Union including the document update finishes within 1 s.
47. Given two operands of 1,000 nodes each, then the time from the button press to the repainted canvas is at most 150 ms.
47a. Given an operation is running (from activation until the result is shown), then the window shows the wait cursor, the Boolean section is `aria-busy`, and all other presses, keys and activations, including a second click on the same button and Escape, are ignored. The busy state is painted before the kernel call starts. If a call runs longer than 150 ms off the UI thread, the notice slot shows "<Operation>: working..." until the result notice replaces it.

## Proposals: preview before applying (Question 6)

**Priority Should. Recommended by UX and the product owner; lead default: include it as the last, optional PR of the slice (PR 4); the customer may drop it.** It is not part of the slice's Must scope. It exists because there is no undo: the maker cannot try an operation and take it back, and without a preview the honest advice would be "save before you combine".

P1. Given a valid selection, when the pointer has rested on an enabled button of the Boolean section for 100 ms, or that button has keyboard focus that came from the keyboard, then the outline of the result (every outline of a compound result, holes included) is drawn over the canvas as a hollow 1.5 px `--preview-new` outline with the white casing (design system, "Blue new, black old"), while the operands stay drawn as they are. It disappears in the frame in which the pointer leaves, the focus moves, the button is activated or Escape is pressed. A result that arrives after that is dropped and never drawn. Nothing is stored, exported or selected by it.
P2. Given the preview, then it is only computed while the operand nodes total at most 2,000; above that the button works without preview and the tooltip note reads "Too large to preview. Replaces the selection. No undo yet." No preview is drawn for a dimmed button. The tooltip opens to the right of the rail and may cover part of the canvas where the operands are; the preview appears 300 ms before the tooltip (100 ms against 400 ms), and the tooltip is short text, so this overlap is accepted. The preview never delays the pointer by more than one frame (the computation runs after the frame, not before it).
P3. Given an operation that would be refused, then no preview is drawn and the refusal notice appears only on activation. When the preview found the result empty, the tooltip note reads "Would be empty. Nothing would change."

## Out of scope

- **Offsetting and V-carving.** `R-EDIT-007` and `R-MFG-CNC-002` come later and build on the same kernel (ADR 0003 §4, §5).
- **Stroke to path.** A stroke never takes part in the region. Making a wide stroke into an outline is its own operation.
- **Groups, layers, text, images.** None exist yet. When `layers-and-grouping` ships, booleans on groups need their own decision (Inkscape refuses groups; its wishlist asks for them). Until then the operation needs only paths and primitives.
- **Division, Cut Path, Combine, Break Apart.** Proposal for a follow-up: Break Apart that keeps holes with their outer outline (Inkscape's does not), Division and Cut Path for splitting a part along a line. Combine (several objects into one compound path without changing geometry) becomes cheap once compound paths exist.
- **Curve refitting after a boolean.** Proposal for a follow-up: fit Bézier curves back to the straight segments within the kernel tolerance, so results have fewer nodes and are easier to edit. ADR 0003 names it as the mitigation. It also belongs with R-VEC-003.
- **Node editing of compound paths** (Question 7), including Join and Split on them.
- **Z-order commands** (bring to front, send to back, raise, lower). Proposal for a small follow-up. Until then stacking order is creation order, and Reverse difference covers the most common need.
- **Undo and redo** (`0020-undo-redo`, not started); this slice only guarantees one labelled commit per operation (criterion 28).
- **Keyboard shortcuts** (none for now, Question 3), a native "Path" menu, a context menu entry. The rail section is the only route.
- **Boolean commands in the Select bar.** The customer moved them to the rail (2026-10-09).
- **Keeping the originals, a "keep originals" modifier, live (non-destructive) boolean effects** like Inkscape's path effects.
- **A setting for the kernel tolerance.** It is fixed (0.01 mm) and not a user option (`CLAUDE.md` §5: no speculative options).
- **Progress display and cancel** for long operations. The budget in criteria 44 to 47 is the control.
- **What jobs do with compound paths.** `manufacturing-roles` and `laser-job-preview-and-output` treat each outline as a contour later; SVG import and export (`svg-import-export`) write a compound path as one `path` with several subpaths.

## Open questions

Questions 1 and 2 and the rail placement are decided. The others have a default; nothing blocks.

**Decided by the customer, 2026-10-09 (final):**

- **Question 1, compound paths in the document model (criteria 20, 31 to 38b): A, approved, including the document-model and file-format change.** A path object may hold several closed outlines, as ADR 0002 §6 already says (stored as the first outline plus an optional `extra_subpaths` list, each anchor with its own id); a result with a hole or separate pieces is one object that fills correctly. Costs a `format_version` bump (to `main`'s current value plus one at merge) and touches rendering, hit-testing, transforms, the project file and the audit places in the table above. (Rejected option B: every outline its own object; a filled ring would become two filled discs.)
- **Question 2, Exclusion and Reverse difference: yes (criteria 13, 13a).** Five operations. Reverse difference is Difference with the top-most object first.
- **Placement:** all boolean operations are a command section in the left tool rail, under the existing tools, always visible, dimmed when not enough is selected or when the Select tool is not active (criteria 1 to 3). Not in the Select bar.

**Open, with defaults:**

3. **Keyboard shortcuts.** *A (default):* none until `undo-redo` exists, as for Object to path: a destructive command should not sit on a key. Also Ctrl+Plus and Ctrl+Minus zoom the page in a browser or web view and are easy to hit by accident. *B:* Inkscape's keys now: Union Ctrl++, Difference Ctrl+-, Intersection Ctrl+*, Exclusion Ctrl+^. Recommendation: A, then B together with undo.
4. **Open paths (criterion 15).** *A (default):* refuse, as R-EDIT-003 ("closed paths") and LightBurn do. *B:* close the path implicitly with a straight line from its last node to its first, as Inkscape does. Recommendation: A. The maker closes it with Join in the Node tool.
5. **The operands after a successful operation (criterion 19).** *A (default):* they are replaced by the result, as in Inkscape and LightBurn; the tooltip says so (criterion 2). *B:* they stay, the result is added on top and selected, and the maker deletes the rest. B is the only safe route without undo but clutters every use. Recommendation: A, with the preview (Question 6) as the safety net.
6. **Preview before applying (Proposals P1 to P3).** *A (default, lead's decision 2026-10-09):* build it in this slice as the last, optional PR (PR 4). *B:* not now; a follow-up once the maker has tried the buttons. Recommendation: A, because it is the substitute for undo, and UX recommends it too. It is separable from everything else here; the customer may drop it.
7. **Node editing of compound paths (criterion 38).** *A (default):* not in this slice. The Node tool shows the outline and says so. *B:* the Node tool edits every node of every outline (move, handles, kind, insert, delete); Join and Split stay off. Recommendation: B soon, as its own slice, because moving a node of a plate with holes is a normal next step; A keeps this slice smaller.
8. **Curves are lost (User value).** Informational; ADR 0003 accepted it. Default: plan the follow-up "curve refit after boolean" after this slice, if the customer finds the polygons annoying in the Node tool.

Decided by the product owner (change if you disagree): the stacking order decides, not the click order; the result takes the base operand's style and place; the result is selected; a refused operation changes nothing; the kernel tolerance is 0.01 mm; any number of operands from two up.

## UX notes

By `ux-engineer`, 2026-10-09; rewritten the same day for the customer's decision that all boolean operations leave the Select bar and live in their own section of the left tool rail, always visible, dimmed when fewer than two objects are selected or the Select tool is not active. Numbers, tokens and component rows are in `docs/design-system.md` ("Boolean tool section", "Boolean glyphs", "Boolean tooltip", "Boolean preview", "Action notice", "Refusal outline", "Compound path in the UI", "Select bar layout"). The criteria above were rewritten on 2026-10-09 to match these notes, with two overrides of the architect's decision (below): the buttons are enabled only with the Select tool active, and the Pen rule is dropped. Where a note and a criterion differ, the criterion stands.

### 1. Where the commands live

- **The Boolean section of the tool rail, and nowhere else.** No Select-bar group, no native menu item, no context-menu entry (none exists). One route means "No undo yet." is said in one place and nothing invites a shortcut (criterion 3 stays). The Select bar has no boolean group any more; its order is settings, kind groups, "Object to path".
- **Position.** The same floating card as the tools (`ToolRail.tsx`), under Select, Pen, Node, Rectangle, Ellipse and Polygon/star, behind a divider: 1 px high, 24 px wide, centred, `--toolbar-icon` at 25 %, 4 px margin above and below on top of the card's 4 px gap (17 px from the last tool to Union). One card, not two: one shadow, one block in the Tab order, one anchor for the notice.
- **Always rendered**, whatever the tool or the selection. Nothing in the rail appears, disappears or moves with the selection (the no-layout-shift rule, and the reason the customer asked for it). The buttons dim instead (section 4).
- `role="toolbar"`, `aria-orientation="vertical"`, accessible name "Boolean operations", no visible title (the glyphs and tooltips carry it).
- **Height.** The card grows from 268 px to 501 px (6 tools, divider, 5 buttons); with the 12 px inset it ends 513 px below the viewport top, inside the 546 px viewport of an 800 x 600 window, with 33 px to spare. Measure at build. A seventh tool or a shorter window will clip it; that is open question 1.

### 2. Buttons, glyphs, order

- **Five buttons, 40 x 40 px, `rounded-md`, 4 px apart**, exactly the size and spacing of the tool buttons as built (`size-10`, `gap-1`, 48 px wide card, 4 px padding top and bottom). Order top to bottom: **Union, Difference, Intersection, Exclusion, Reverse difference** (all five are in the slice, customer decision on Question 2). Names are exactly these; "Reverse difference" is never shortened.
- **Glyph size: 20 px on the rail, not 16.** The tool icons are 20 px (Lucide `size={20}`); a 16 px glyph in a 40 px button would be the smallest thing in the rail and the 4 px empty or solid overlap interior is the limit of legibility. The artwork stays the 16 x 16 viewBox of the design system, scaled by 1.25, stroke kept at 1.5 px absolute (`vector-effect: non-scaling-stroke`), so the overlap interior is 5.4 px: Exclusion (empty overlap) and Intersection (solid overlap) stay apart at 1x. Check at 1x and 2x at build.
- **Glyph convention** (Inkscape's): two overlapping rounded squares, the first-drawn one **upper left (the lower object)**, the second **lower right (the upper object)**. The result region is solid `currentColor`; the parts that disappear are outline only. Union: both solid. Difference: upper-left solid minus the overlap, lower-right outline. Intersection: both outlines, overlap solid. Exclusion: both solid, overlap empty. Reverse difference: lower-right solid minus the overlap, upper-left outline. Difference and Reverse difference are mirror images; the cue is which square is solid, which is why the tooltips and the preview matter. Geometry in the design-system row "Boolean glyphs".
- **Colours.** `--toolbar-icon` glyph, hover ground `--editor-accent-hover`, focus-visible 2 px `--editor-accent` ring with 1 px `--toolbar-bg` offset. The tool buttons have no hover ground today and use the grey `--ring` (2.4:1 on `--toolbar-bg`); in the same PR they get the same hover ground and ring (class change only, no behaviour change) so the rail does not carry two focus looks.
- **Shortcut display: none.** The accessible name is the bare operation name ("Union", not "Union (...)"), the tooltip has no key hint.

### 3. Commands, not tools

The Boolean buttons run and return. They never call the tool switch, never change the active tool, carry no `aria-pressed` and never show `--toolbar-icon-active-bg`: the solid blue button in the rail is the active tool, always exactly one, and a command must not steal it. The only pressed look is the transient `:active` ground; while an operation is busy the pressed button keeps that ground, and it is gone when the operation ends. Nothing persists, nothing is remembered.

### 4. Enabled states

| Situation | Buttons |
|---|---|
| 0 or 1 objects selected, or any tool other than Select active | All five `aria-disabled="true"`, 40 % opacity, no hover ground, default cursor, **still focusable**. Tooltip note "Select two or more closed objects." (with a tool other than Select active, followed by "Use the Select tool."). Activation does nothing and writes nothing; the tooltip stays open on a press (it is the explanation). |
| 2 or more selected with the Select tool active, any mix of kinds, any closed or open | **Enabled, full contrast, never pre-dimmed.** Dimming means "not available now" and nothing else; an open path is not that, because pressing gives the useful answer (which object, in red). |
| 2 or more selected, at least one open path | Enabled. Tooltip note "Needs closed paths: 2 of 3 selected are open." (information, not a lock). Activation is refused with the alert and red outlines of section 6 (criterion 15), the same for a mouse and a key. |
| An operand with no area, or an empty result | Enabled; not known without running the kernel; refused on activation (criteria 16, 17). |
| An operation is running | `aria-busy="true"` on the toolbar; further activations ignored. |

This is the one place in the product where a control is dimmed and not removed (`docs/design-system.md` otherwise says "Hidden, not disabled"): the rail is a fixed-position block that must not move, and the customer asked for it. The panel and the Select bar keep their rule.

**Interaction with the active tool (architect decision 2026-10-09, overrides the earlier "any tool" rule).** Enabled when two or more objects are selected **and the Select tool is active**. With the Node or Pen tool the object selection is kept but not drawn, and the command deletes objects with no undo, so it must act only on a selection the maker can see; Rectangle, Ellipse and Polygon/star clear the selection on entry (`Session::set_tool`). The buttons are therefore dimmed in all three cases, with the tooltip hint "Use the Select tool." The state comes from one function, `boolean_availability` in `curvyo-ui-core`; the frontend only displays it. The Pen rule "Finish the path first." is dropped: with Pen active the buttons are dimmed anyway and the generic note applies. After a success the **active tool is unchanged** (it is the Select tool by necessity, criterion 23). A result that is a compound path shows the pill of criterion 38 if the maker then switches to the Node tool; an ordinary one-outline result is node-editable there as before.

### 5. Keyboard

- **Tab order.** Unchanged: rail, canvas, bars. The six tool buttons keep their individual Tab stops (accepted behaviour; making the whole rail one roving toolbar is a separate change, open question 2). The Boolean section is **one more Tab stop with roving focus**: Tab lands on the button that last had focus (Union the first time); Up and Down move, Home and End jump, no wrap; moving never activates; Space and Enter activate; Left and Right do nothing. Dimmed buttons stay in the roving set. Five more Tab stops would make the rail eleven.
- **Letters** are not used and are ignored while focus is in the section (the existing "focused control" rule). `B` is the Pen; Ctrl+Plus and Ctrl+Minus zoom a browser page (criterion 3).
- **Focus after activation**, the rail's existing rule (`onReturnFocus`): a **mouse** press returns focus to the canvas (so the tool letters work again), after a success and after a refusal. A **key** activation leaves focus on the pressed button, after a success and a refusal. Nothing unmounts, so focus is never lost; after a success the buttons dim (one object is selected) and focus rests on a dimmed, focusable button, which is allowed. This replaces the earlier "focus moves to the canvas after a success" (criterion 22a): that rule existed only because the old group left the tree.
- Tooltips open on keyboard focus.

### 6. Result feedback: where it appears

- **Anchor.** A text chip to the right of the rail, over the canvas, **top edge level with the top of the Union button, left edge 12 px right of the rail card** (72 px from the viewport's left edge, the same left edge as the bars; implemented as an absolutely positioned child of the section wrapper, so it follows the rail). Why here: the eye and the pointer are on the rail; the tooltip closes on the press, so the notice replaces it in the same place; it does not depend on the active tool (the Select bar exists only with the Select tool); and the 80-character refusal needs room. The status bar was rejected: it is display only, 24 px high, holds the cursor readout that changes every frame, and sits far from the rail. The pointer-anchored hint chip was rejected for the old reason (it would cover the neighbouring buttons). It clamps inside the viewport; at 800 x 600 the viewport is about 496 px wide, so a 360 px notice at 72 px fits.
- **Success.** The result replaces the operands in place and is the only selected object (criterion 23); the Properties panel switches to it; no flash, no animation; the buttons dim because one object is selected. The **success notice** says "Union: 3 objects became 1 path. No undo yet." or "... became 1 compound path." One line, 3 s or until the next action, `role="status"`, no focus.
- **Refusal.** Nothing changes; the selection, the tool and the section stay. The **refusal notice** (the alert look of the validation chip) stays 8 s or until the next action (press anywhere, key, selection change, tool change; a pointer move or hover over another button does not clear it). `role="alert"`. A second activation shows it again. If a tooltip opens over it, the tooltip wins (it is hover intent); the notice is still there afterwards.
- **Which object is the problem.** For open paths and for objects without area, each offending operand is redrawn over the canvas as a hollow 2 px `--field-invalid` outline with the white casing for as long as the notice is on screen. Nothing is stored or selected by it (design system, "Refusal outline").
- **Wording** (every refusal ends with "Nothing was changed."; `<Op>` is one of the five names):
  - open: "Union needs closed paths. 1 of 3 selected objects is open. Nothing was changed." (plural: "2 of 3 selected objects are open.")
  - no area: "Union needs shapes that enclose an area. 1 of 3 selected objects has no area. Nothing was changed."
  - empty: Intersection "Intersection is empty: the selected objects share no area."; Difference "Difference is empty: the lowest object is covered completely."; Reverse difference "Reverse difference is empty: the top object is covered completely."; Exclusion "Exclusion is empty: no area is covered an odd number of times."
- No modal dialog anywhere (criterion 18). The two live regions (status, alert) are in the tree from the start, empty, so the announcement is the text change.

### 7. Tooltips

Radix `Tooltip` as the other rail buttons: 400 ms, **`side="right"`**, 6 px offset, `bg-popover`, `text-xs`, ring; three lines: the name (semibold), the rule, a muted note; max width 260 px. The name and rule never wrap; the note may wrap to a second line. No shortcut text.

| Button | Rule line |
|---|---|
| Union | Everything covered by any selected object. |
| Difference | The lowest selected object minus the others. |
| Intersection | Only what every selected object covers. |
| Exclusion | Areas covered an odd number of times. |
| Reverse difference | The top selected object minus the others. |

Note line by state, first match wins: fewer than two selected, or a tool other than Select active, "Select two or more closed objects." (plus "Use the Select tool." when the tool is not Select); open path "Needs closed paths: 2 of 3 selected are open."; preview computed and empty "Would be empty. Nothing would change."; too large to preview "Too large to preview. Replaces the selection. No undo yet."; otherwise "Replaces the selection. No undo yet." "Lowest" and "top" are by stacking order; the preview is how the maker finds out which that is, since no z-order command exists yet.

### 8. No undo, and the preview (Question 6)

What the interface says: before the press, the note line, "No undo yet." and the preview; after it, the same two words in the success notice. When `undo-redo` ships, both mentions are deleted and nothing else here changes.

**Recommendation for Question 6 stays A, build the preview in this slice (PR 4, optional).** It is the only protection a command that replaces N objects has without undo, and Difference and Reverse difference are mirror images of an invisible stacking order. Look (P1 to P3, unchanged): the result's outlines (every outline of a compound result) as a hollow 1.5 px `--preview-new` line with the white casing, drawn above the operands, which stay as committed, no fill, constant screen width, below the selection box and handles. It starts after 100 ms of rest on an enabled button that would not be refused, or at once when a button gains focus by keyboard (Tab into the section or an arrow key); it ends in the frame the pointer leaves, the focus moves, the button is activated or Escape is pressed. No preview for a dimmed button, for an open-path selection, over 2,000 operand nodes, or for an operation that would be refused.

**New with the rail:** the tooltip now opens to the right of the rail, over the left part of the canvas, where operands often are. The preview appears 300 ms before the tooltip, and the tooltip is text-only and short-lived, so this is accepted. The note "Would be empty. Nothing would change." lives in the tooltip, which is why the tooltip is not suppressed while a preview is showing.

### 9. Long operations

The budgets (criteria 44 to 47) are the control; no progress bar, no cancel (out of scope). From the press until the repainted result: cursor `wait` over the whole window, `aria-busy` on the toolbar, the pressed button keeps its pressed ground, all other presses, keys and activations are ignored (including a second press on the same button and Escape). The busy state is painted before the kernel call starts (yield two frames: one `requestAnimationFrame` runs before the paint, not after it). If a call runs longer than 150 ms off the UI thread, the notice slot shows "Union: working..." (`role="status"`), replaced by the result notice. Other buttons are not dimmed while busy: dimming means "not enough selected or not the Select tool" and nothing else.

### 10. Keyboard map and accessibility

- Shortcuts: none (criterion 3). Reached by Tab (the section), Up and Down inside it, Space or Enter. The design-system table "Keyboard shortcuts established so far" has a row "none" until `undo-redo`.
- Names are the five operation names; the rule and note lines are the description (`aria-describedby`, also on focus).
- Non-text contrast: glyph 8.3:1 on `--toolbar-bg`; the dimmed state is exempt but stays focusable and announced as dimmed (`aria-disabled`).
- Hit target 40 x 40 px; 4 px gap.
- State is never colour alone: the open-path cause is text; the red outline is paired with the notice; dimming is paired with the tooltip note.
- No motion exists, so `prefers-reduced-motion` needs nothing.

### 11. Compound paths in the UI (criteria 31 to 38)

- **Properties panel subject line:** "Compound path", "3 compound paths" when every selected object is one, "N paths" when mixed with ordinary paths, "N objects" when mixed with primitives. The Style sections are those of a path. The Markers block is removed (hidden, not disabled) while a compound path is selected, until markers on compound paths are decided (default; the architect flagged `0018`).
- **Select bar** for a compound path: no kind group; "Object to path" is not shown (criterion 36). The Boolean section of the rail is independent of the kind.
- **Node tool (criterion 38):** with a compound path as the only selected object the Node bar slot shows a text-only pill (the bar surface, no controls): "Nodes of compound paths cannot be edited yet." (`role="status"`, so a screen reader hears it on entry). The outline is drawn, no node, handle or segment overlay. **Double-click on a compound path in the Select tool** does not switch tools (the precedent of the primitives): the shared hint chip shows the same sentence for 3 s. Pressing `N` or the rail button still enters the Node tool and shows the pill.
- Click inside a hole picks what is behind (criterion 33): the hover box follows what a press would pick, so over a hole it is the object behind that lights up.

### Criteria changes

All applied on 2026-10-09 to criteria 1, 1a, 2, 3, 13, 13a, 15, 22a, 23, 47a and P1, and to Questions 1 and 2. Differences from the original request: the buttons are enabled only with the Select tool active (architect decision), and the Pen rule "Finish the path first." was dropped.

### Open design questions (defaults taken)

1. **Rail height.** 501 px with the section; fits an 800 x 600 window (513 of 546 px). Default: no responsive rule now; when a seventh tool arrives or the window is shorter, the rail becomes two columns or scrolls (decide then).
2. **Roving focus for the whole rail.** Default: not now; the six tool buttons keep their six Tab stops, the Boolean section is one. A single vertical toolbar for all 11 buttons is the cleaner model and a separate change to accepted behaviour (customer).
3. **Press on a dimmed button.** Default: nothing, the tooltip explains. Alternative: a one-line status notice "Select two or more closed objects."
4. Markers block on compound paths. Default: hidden. Decide with the markers owner.
5. Preview on keyboard focus. Default: yes, keyboard-initiated focus only.

## Flags for the architect

- Document model: compound path (several closed outlines per object; ADR 0002 §6). One-outline assumptions to revisit: `PathSnapshot` (`closed` flag plus a flat anchor list), `path_codec`, `interior::contains_point`, the users of `segment_bounds`, `rotated`/`scaled`/`sheared`, the Node tool, Join and Split (`0006-path-merge-split-and-node-types`), `0018-stroke-markers` (start and end of a path), `0014-advanced-selection` hit-testing, `0019-multi-object-transform`.
- Kernel: `i_overlay` (the spike chose `clipper2-rust`, replaced on 2026-10-09, see `adrs.md`); fill rule nonzero on input and output; fixed tolerance and grid; canonical outline order and winding (criteria 41, 43).
- Format: `format_version` is `main`'s current value plus one at merge (`main` is at 7).
- No new crate; `curvyo-geometry-core` gets the boolean module; one new dependency, `i_overlay`, pinned exactly.
- Settled in `adrs.md` (2026-10-09): encoding, audit table, kernel pipeline, commit labels, code locations. Nothing further is open for the architect.

## Build order

From the architect's PR split (`adrs.md`): PR 1 is the kernel, in `curvyo-geometry-core` only, and shares no crate with the model PR of `0015-document-size-and-rulers`, so the two can run in parallel. PR 2 (compound path), PR 3 (command and UI) and PR 4 (preview, optional) touch the same crates as the rulers PRs and are staggered with them: they start after the rulers feature has merged. Whichever feature merges first takes `main`'s `format_version` plus one, and the other rebases. Rulers needs no version bump of its own.

## Links

Requirements: R-EDIT-003, R-EDIT-004 (Object to path), R-SYS-003 (same result on every OS).
Related: `docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md`, `docs/adr/0002-document-model-units-and-svg-round-trip.md`, `specs/0002-path-node-editing/specification.md` (one outline per path), `specs/0003-primitive-shapes/specification.md` (Object to path), `specs/0007-stroke-and-fill-styling/specification.md` (nonzero fill, styles), `specs/0014-advanced-selection/specification.md` (hit order), `docs/design-system.md` ("Select bar", "Blue new, black old").
PR: none yet.
