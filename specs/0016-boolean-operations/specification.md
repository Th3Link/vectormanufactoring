# Boolean operations

Status: Ready
Priority: Must
Origin: Customer (R-EDIT-003, in the confirmed MVP cut). Everything marked **Proposal** is the product owner's idea and is not accepted until the customer says so.

## User value

As a maker I want to combine, cut and overlap shapes (union, difference, intersection) so that I get one clean closed outline for my laser, for example a plate with mounting holes, a bracket from two rectangles, or a keyhole from a circle and a slot, without redrawing it by hand.

**What we match from Inkscape and LightBurn, and what we do differently:**

- **Same direction rule as Inkscape: Difference is the bottom object minus the objects above it, and the result keeps the bottom object's look.** Inkscape's manual says the stacking order decides which object the operation applies to. LightBurn uses the order in which you clicked and tells you to undo and run it again to flip it. We use the stacking order only, so the same drawing always gives the same result, however the objects were selected.
- **Any number of objects in one step.** Inkscape (through 1.2) accepts exactly two paths for Difference, Exclusion, Division and Cut Path; LightBurn's Boolean tools take exactly two shapes and its Weld takes more. Here Union, Difference and Intersection take two or more.
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
| Exclusion | What an odd number of operands cover (for two operands: covered by one, not both) | **Proposal**, default in (Question 2) |
| Reverse difference | The top-most operand minus everything covered by the other operands | **Proposal**, default in (Question 2) |
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

1. Given the Select tool is active and two or more objects are selected (any mix of path, rectangle, ellipse, polygon and star), then the Select bar shows a Boolean group with the buttons Union, Difference and Intersection (plus Exclusion and Reverse difference if Question 2 is accepted). Given no object or one object is selected, then the group is not shown.
2. Given the Boolean group is shown, then the group is one Tab stop with roving focus (Left, Right, Home and End move the focus without activating; Space or Enter activates), and every button has an accessible name equal to its operation name and a tooltip of three lines: the name, the rule (for Difference: "The lowest selected object minus the others.") and a note. The note reads "Replaces the selection. No undo yet." in the normal case, and "Needs closed paths: 2 of 3 selected are open." when an open path is selected. Given an open path is selected, then all buttons are dimmed and `aria-disabled` but stay focusable, and activating one shows the refusal of criterion 15, the same as a mouse click would.
3. Given any state of the app, then no keyboard shortcut and no menu item starts a boolean operation (default of Question 3). Object to path has no shortcut either, for the same reason: there is no undo yet.

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
13. **Proposal.** Given two or more operands, when the maker activates Exclusion, then the result region is covered by an odd number of operands. When the maker activates Reverse difference, then it is the top-most operand's region minus the union of the others.
14. Given random polygon pairs A and B with straight edges in the test suite (at least 200 pairs, fixed seed, including pairs with holes and self-intersections), then the areas obey area(A ∪ B) + area(A ∩ B) = area(A) + area(B) and area(A − B) + area(A ∩ B) = area(A), each within the bound `1e-6 * (area(A) + area(B)) + 0.001 mm * (perimeter(A) + perimeter(B))`, where the perimeter is the total length of all outlines of the operand's region. The second term is one grid pitch (criterion 39) of boundary movement along the operands' edges: snapping to the grid and the node removal of criterion 24 move the boundary by up to that much, so a pure 1e-6 bound cannot hold on the 0.001 mm grid. The same bound applies to the identity for Exclusion, area(A ⊕ B) = area(A) + area(B) − 2·area(A ∩ B). Pairs whose coordinates are whole multiples of 0.1 mm and have no feature smaller than 0.002 mm hold the identities at 1e-6 relative alone (test: at least 100 such pairs).

### Refusals: nothing changes

"Nothing changes" means: the same objects with the same ids, order, geometry and styles, the same selection, the same active tool, no commit written to the document.

15. Given any operand is an open path, when the maker activates any operation, then the operation is refused and a refusal notice appears under the Boolean group for 8 seconds or until the next press, key, selection change or tool change. It reads "<Operation> needs closed paths. 1 of 3 selected objects is open. Nothing was changed." (plural: "2 of 3 selected objects are open."). While the notice is shown, each offending operand is drawn with a hollow 2 px red outline (`--field-invalid`, with the white casing) over the canvas; the outline is not stored and does not change the selection.
16. Given any operand encloses no area (all nodes on one line, or all nodes at one point, for example a closed path of two nodes), then the operation is refused with the notice "<Operation> needs shapes that enclose an area. 1 of 3 selected objects has no area. Nothing was changed.", shown and outlined as in criterion 15.
17. Given the result region is empty, then the operation is refused with one of these notices: Intersection "Intersection is empty: the selected objects share no area."; Difference "Difference is empty: the lowest object is covered completely."; Reverse difference "Reverse difference is empty: the top object is covered completely."; Exclusion "Exclusion is empty: no area is covered an odd number of times."; each followed by "Nothing was changed." Test cases: Intersection of two discs that do not touch; Intersection of two squares that share only an edge or only a corner; Difference where the top object covers the bottom object completely; Exclusion of two identical squares.
18. Given a refusal, then no modal dialog opens, the refusal notice is an alert (`role="alert"`), the buttons stay usable, and a second activation gives the same result. A mouse activation returns focus to the canvas; a keyboard activation leaves it on the button.

### The result

19. Given a successful operation, then all operands are removed and exactly one new object takes their place. Objects that were not selected keep their ids and their order relative to each other.
20. Given the result region has one outline and no holes, then the new object is an ordinary closed path that the Node tool edits like any other path. Given the result has a hole or several separate pieces, then it is one compound path object, not several objects.
21. Given a successful operation, then the new object's complete style (stroke on or off, width, colour, opacity, dash, join, cap; fill on or off, colour, opacity) is a copy of the base operand's style.
22. Given a successful operation, then the new object sits at the base operand's place in the stacking order, directly where that operand was. Objects between the operands that were not selected stay where they are.
22a. Given a successful operation by mouse or keyboard, then keyboard focus is on the canvas (the Boolean group leaves the tree with the selection of one object, so focus must not fall back to the page).
23. Given a successful operation, then the new object is the only selected object, the Select tool is still active, and the Properties panel describes it (subject line "Compound path" when the result is one).
24. Given a successful operation, then every node of the result is a Corner node with no handles, no two consecutive nodes are closer than 0.001 mm, and no node lies on the straight line between its neighbours (within 0.001 mm). Test: the union of two squares of side 20 mm that share a full edge has 4 nodes.
24a. Given any operation, then removing nodes (criterion 24) never moves the boundary by more than one grid unit (0.001 mm) from the outline it is applied to: every point of the cleaned outline lies within 0.001 mm of the outline before cleanup, and every point of the outline before cleanup lies within 0.001 mm of the cleaned one. This is the measurable form of criteria 24 and 25 for the cleanup step; it holds for every outline of the result, holes included.
24b. Given an operand polyline with many nearly collinear vertices (test: 100,000 vertices on a circle of radius 10 mm, united with a far-away square), then the cleaned outline keeps its shape: every point of it lies within 0.001 mm of the input polyline (after snapping to the grid) and vice versa, with no cumulative drift however many consecutive vertices are removed, and its area differs from the input polygon's by at most 0.001 mm times the outline length. It completes within the budget of criterion 45 (the input counts as 100,000 nodes; the 2 s budget scales with the node count, so this test allows 20 s).
25. Given an operand with curves, then the result's outline lies within 0.01 mm of the exact boolean result of the operands' curves. Test: the union of two discs of radius 10 mm with centres 5 mm apart; every node of the result lies within 0.01 mm of the true outline, and the straight segments between nodes deviate from it by at most 0.01 mm.
26. Given a disc of radius 10 mm (4 Bézier segments) and a far-away rectangle as operands of a union, then the disc's outline in the result has between 71 and 142 nodes. This is the node budget: the fewest straight segments that keep every chord within 0.01 mm, and at most twice that.
27. Given a successful operation, then the new object's rotation is 0, so its selection box is aligned with the document axes.
28. Given a successful operation, then the document records exactly one commit, whose label names the operation (stored as `boolean_union`, `boolean_difference`, `boolean_intersection`, `boolean_exclusion`, `boolean_reverse_difference`; the undo slice maps them to display text). The operands are never removed without the result being added, and the reverse never happens either, however the operation ends. This is what lets `undo-redo` treat it as one step later.
29. Given a successful operation, then a transient notice of one line names the operation and the counts and ends with "No undo yet.", for example "Union: 3 objects became 1 path. No undo yet." or "Difference: 2 objects became 1 compound path. No undo yet." It disappears within 3 seconds or on the next action, takes no focus and is a `role="status"` live region.
30. Given a result is saved and the project reopened, then the object has the same outlines, node order and style as before closing.

### Compound paths

Today a path object holds exactly one outline (`0002-path-node-editing`, Out of scope: compound paths; ADR 0002 §6 already plans "explicit open/closed subpaths"). A difference with a hole inside, or a union of two separate shapes, cannot be one object without more than one outline per object. This part is the minimum that makes such a result usable. It is a document-model and file-format change, so per `CLAUDE.md` §3 it needs the customer: **proposed, awaiting customer approval (Question 1)**. The architect recommends it and designed it: a path keeps its first outline in the existing keys and stores further outlines in one new optional list, `extra_subpaths`; every anchor of every outline has its own unique id; the fill rule is nonzero over all outlines of the object together. The architect's audit (`adrs.md`, "The audit") lists every place that assumes one outline. Each place needs its own test; the criteria below cover them as follows:

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
47a. Given an operation is running (from activation until the result is shown), then the window shows the wait cursor, the Boolean group is `aria-busy`, and all other presses, keys and activations, including a second click on the same button and Escape, are ignored. The busy state is painted before the kernel call starts. If a call runs longer than 150 ms off the UI thread, the notice slot shows "<Operation>: working..." until the result notice replaces it.

## Proposals: preview before applying (Question 6)

**Priority Should. Recommended by UX and the product owner; lead default: include it as the last, optional PR of the slice (PR 4); the customer may drop it.** It is not part of the slice's Must scope. It exists because there is no undo: the maker cannot try an operation and take it back, and without a preview the honest advice would be "save before you combine".

P1. Given a valid selection, when the pointer has rested on a Boolean button for 100 ms, or the button has keyboard focus that came from the keyboard, then the outline of the result (every outline of a compound result, holes included) is drawn over the canvas as a hollow 1.5 px `--preview-new` outline with the white casing (design system, "Blue new, black old"), while the operands stay drawn as they are. It disappears in the frame in which the pointer leaves, the focus moves, the button is activated or Escape is pressed. A result that arrives after that is dropped and never drawn. Nothing is stored, exported or selected by it.
P2. Given the preview, then it is only computed while the operand nodes total at most 2,000; above that the button works without preview and the tooltip note reads "Too large to preview. Replaces the selection. No undo yet." The preview never delays the pointer by more than one frame (the computation runs after the frame, not before it).
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
- **Keyboard shortcuts** (Question 3), a native "Path" menu, a context menu entry.
- **Keeping the originals, a "keep originals" modifier, live (non-destructive) boolean effects** like Inkscape's path effects.
- **A setting for the kernel tolerance.** It is fixed (0.01 mm) and not a user option (`CLAUDE.md` §5: no speculative options).
- **Progress display and cancel** for long operations. The budget in criteria 44 to 47 is the control.
- **What jobs do with compound paths.** `manufacturing-roles` and `laser-job-preview-and-output` treat each outline as a contour later; SVG import and export (`svg-import-export`) write a compound path as one `path` with several subpaths.

## Open questions

Each has a default; nothing blocks.

1. **Compound paths in the document model (criteria 20, 31 to 38b).** **Proposed by the product owner and the architect, awaiting the customer's approval (it changes the document model and the file format).** *A (default, recommended):* a path object may hold several closed outlines, as ADR 0002 §6 already says (stored as the first outline plus an optional `extra_subpaths` list, each anchor with its own id); a result with a hole or separate pieces is one object that fills correctly. Costs a model change and a `format_version` bump (to `main`'s current value plus one at merge), and touches rendering, hit-testing, transforms, the project file and the audit places in the table above. The kernel PR does not depend on this answer. *B:* every outline of the result becomes its own closed path object. No model change, but a filled ring becomes two filled discs (the hole is painted over), and a union of separate pieces is several objects. Only workable for cut-line-only work. Recommendation: A.
2. **Exclusion and Reverse difference in this slice (criterion 13).** *A (default):* yes, both; they reuse the kernel and add two buttons, and Reverse difference makes up for the missing z-order commands. *B:* only Union, Difference, Intersection now. Recommendation: A.
3. **Keyboard shortcuts.** *A (default):* none until `undo-redo` exists, as for Object to path: a destructive command should not sit on a key. Also Ctrl+Plus and Ctrl+Minus zoom the page in a browser or web view and are easy to hit by accident. *B:* Inkscape's keys now: Union Ctrl++, Difference Ctrl+-, Intersection Ctrl+*, Exclusion Ctrl+^. Recommendation: A, then B together with undo.
4. **Open paths (criterion 15).** *A (default):* refuse, as R-EDIT-003 ("closed paths") and LightBurn do. *B:* close the path implicitly with a straight line from its last node to its first, as Inkscape does. Recommendation: A. The maker closes it with Join in the Node tool.
5. **The operands after a successful operation (criterion 19).** *A (default):* they are replaced by the result, as in Inkscape and LightBurn; the tooltip says so (criterion 2). *B:* they stay, the result is added on top and selected, and the maker deletes the rest. B is the only safe route without undo but clutters every use. Recommendation: A, with the preview (Question 6) as the safety net.
6. **Preview before applying (Proposals P1 to P3).** *A (default, lead's decision 2026-10-09):* build it in this slice as the last, optional PR (PR 4). *B:* not now; a follow-up once the maker has tried the buttons. Recommendation: A, because it is the substitute for undo, and UX recommends it too. It is separable from everything else here; the customer may drop it.
7. **Node editing of compound paths (criterion 38).** *A (default):* not in this slice. The Node tool shows the outline and says so. *B:* the Node tool edits every node of every outline (move, handles, kind, insert, delete); Join and Split stay off. Recommendation: B soon, as its own slice, because moving a node of a plate with holes is a normal next step; A keeps this slice smaller.
8. **Curves are lost (User value).** Informational; ADR 0003 accepted it. Default: plan the follow-up "curve refit after boolean" after this slice, if the customer finds the polygons annoying in the Node tool.

Decided by the product owner (change if you disagree): the stacking order decides, not the click order; the result takes the base operand's style and place; the result is selected; a refused operation changes nothing; the kernel tolerance is 0.01 mm; any number of operands from two up.

## UX notes

By `ux-engineer`, 2026-10-09. Numbers, tokens and component rows are in `docs/design-system.md` ("Boolean group", "Boolean preview", "Action notice", "Compound path in the UI", "Select bar layout"). Where a note needs a criterion to change, it is listed in "Criteria changes requested" at the end; until the product owner agrees, the criterion as written stands.

### 1. Where the commands live

- **The Select bar only.** No native menu item, no context-menu entry, no "Path" menu (none exists). A second route would double the places that must say "no undo yet" and would invite a shortcut. This is criterion 3 and stays.
- **Order in the bar:** settings, kind groups, **Boolean group**, "Object to path" last. "Object to path" keeps the last slot (far from "Remove rounding", `unified-object-editing`); the Boolean group sits between because it acts on the whole selection and not on one kind.- **Shown** when two or more objects are selected, of any kind (criterion 1). With one or none it is not rendered (hidden, not disabled, as for every bar group). It wraps as one unit (148 px), never split.
- Group: `role="group"`, accessible name "Boolean operations". No visible title (the glyphs and tooltips carry it; a title would cost about 90 px).

### 2. Buttons, icons, order

- Five icon-only buttons, 28 x 28 px, `rounded-md`, 16 px glyph, 2 px apart, in this order: **Union, Difference, Intersection, Exclusion, Reverse difference**. The order does not change if Question 2 is answered B (the last two simply do not exist). Names are exactly these; "Reverse difference" is never shortened.
- Glyph convention (Inkscape's, redrawn for 16 px): two overlapping rounded squares, the first-drawn one **upper left (the lower object)**, the second **lower right (the upper object)**. The result region is solid `currentColor`, the parts that disappear are outline only (1.5 px). Union: both squares solid, one silhouette. Difference: upper-left solid minus the overlap, lower-right outline. Intersection: both outlines, overlap solid. Exclusion: both solid, overlap empty. Reverse difference: lower-right solid minus the overlap, upper-left outline. Difference and Reverse difference are mirror images; the position of the solid square (lower object upper left) is the cue, which is also why the tooltips and the preview matter. Geometry in the design-system row. No new colour: `--toolbar-icon` glyph, hover `--editor-accent-hover`, focus-visible 2 px `--editor-accent` ring with 1 px `--toolbar-bg` offset.
- Keyboard: the group is **one Tab stop** with a roving focus (Left/Right/Home/End move the focus and never activate; Space or Enter activates). Five Tab stops would make the bar, which already holds up to a dozen, tedious to cross. Tooltips show on focus.

### 3. Enabled states

| Selection | Group | Buttons |
|---|---|---|
| 0 or 1 object | not rendered | |
| 2 or more, all closed, any mix incl. compound paths | shown | all five enabled |
| 2 or more, at least one open path | shown | all five `aria-disabled`, 40 % opacity, still focusable; the tooltip's third line gives the cause; activating one shows the refusal notice of criterion 15 (same text a mouse user would get), so the keyboard route is identical |
| An operand that encloses no area (criterion 16), or an empty result (criterion 17) | shown | enabled in appearance: not known without running the kernel; refused on activation |
| An operation is running | shown | `aria-busy="true"`, further activations ignored |

Only the open-path case is pre-computed (a flag on each object, free). Everything geometric waits for the click or the preview. A refusal never closes or hides the group.

### 4. Tooltips (criterion 2)

Radix `Tooltip`, 400 ms, `side="bottom"`, max width 260 px, three lines: the name (semibold), the rule, a muted note. No line wraps.

| Button | Rule line |
|---|---|
| Union | Everything covered by any selected object. |
| Difference | The lowest selected object minus the others. |
| Intersection | Only what every selected object covers. |
| Exclusion | Areas covered an odd number of times. |
| Reverse difference | The top selected object minus the others. |

Note line, by state: normal "Replaces the selection. No undo yet."; open path "Needs closed paths: 2 of 3 selected are open."; preview computed and the result would be empty "Would be empty. Nothing would change."; selection too large for a preview "Too large to preview. Replaces the selection. No undo yet." "Lowest" and "top" are by stacking order; the preview is how the maker finds out which that is, since no z-order command exists yet.

### 5. Result feedback

- **Success.** The result replaces the operands in place and is the only selected object (criterion 23); the Properties panel switches to it; no flash, no animation. **Focus moves to the canvas** after a mouse or keyboard activation, because the group unmounts with the selection of one (a focused control leaving the tree must not drop focus to `body`). The success **action notice** (design system) says "Union: 3 objects became 1 path. No undo yet." or "... became 1 compound path." One line, 3 s or until the next action, `role="status"`, no focus.
- **Refusal.** Nothing changes; the selection, the tool and the group stay. The **refusal notice** (alert variant of the same component, the look of the validation chip) appears under the group, stays 8 s or until the next action (press, key, selection or tool change). The sentence is long enough that 3 s is too short to read it. A mouse activation returns focus to the canvas as everywhere; a keyboard activation leaves it on the button. `role="alert"`. A second activation shows it again.
- **Which object is the problem.** For open paths and for objects without area, each offending operand is redrawn over the canvas as a hollow 2 px `--field-invalid` outline with the white casing, for as long as the notice is on screen. Nothing is stored or selected by it. This keeps the promise in "User value" ("say which object is open") without changing the selection (criterion 15).
- **Wording** (every refusal ends with "Nothing was changed."; `<Op>` is one of the five names):
  - open: "Union needs closed paths. 1 of 3 selected objects is open. Nothing was changed." (plural: "2 of 3 selected objects are open.")
  - no area: "Union needs shapes that enclose an area. 1 of 3 selected objects has no area. Nothing was changed."
  - empty: Intersection "Intersection is empty: the selected objects share no area."; Difference "Difference is empty: the lowest object is covered completely."; Reverse difference "Reverse difference is empty: the top object is covered completely."; Exclusion "Exclusion is empty: no area is covered an odd number of times."
- No modal dialog anywhere (criterion 18).

### 6. No undo, and the preview (Question 6)

What the interface says: before the click, the third tooltip line, "No undo yet." and the preview; after it, the same two words in the success notice. When `undo-redo` ships, both mentions are deleted and nothing else changes here.

**Recommendation for Question 6: A, build the preview in this slice.** It is the only protection a destructive command that replaces N objects has when there is no undo; Difference and Reverse difference are mirror images and the stacking order is invisible, so the maker cannot tell the outcome from the buttons; the cost is one extra kernel call on hover, limited by P2, and it is separable. Without it the honest instruction would be "save before you combine".

Look (Proposals P1 to P3, defined): the result's outlines (every outline of a compound result, holes included) as a hollow 1.5 px `--preview-new` line with the white casing, drawn above the operands, which stay exactly as they are ("black old"); no fill; constant screen width; below the selection box and handles. Shown while the pointer rests on a button for 100 ms (hover intent, so a sweep across five buttons does not compute five results) or while a button has keyboard focus that came from the keyboard; gone in the frame the pointer leaves, the focus moves, the button is activated or Escape is pressed. Stale results are dropped, never drawn. Over 2,000 operand nodes, or for a refused operation, no preview (the tooltip's note line says why).

### 7. Long operations

The budgets (criteria 44 to 47) are the control; no progress bar (the kernel reports none), no cancel (out of scope). From the activation until the repainted result: cursor `wait` over the whole window, `aria-busy` on the group, all other presses, keys and activations ignored (including a second click on the same button), Escape ignored. The busy state must be painted before the kernel call starts (yield one frame), so the cursor appears even if the call blocks. If the architect runs the kernel off the UI thread and a call takes longer than 150 ms, the notice slot shows "Union: working..." (`role="status"`), replaced by the result notice.

### 8. Keyboard and shortcuts

None (criterion 3). Reachable by Tab (the group), arrows inside it, Space or Enter. Letters are not used: `B` is the Pen, and Ctrl+Plus and Ctrl+Minus zoom a browser page. Candidates to decide with `undo-redo`: not the Inkscape keys in a browser build; the table "Keyboard shortcuts established so far" gets a row "none" until then.

### 9. Accessibility

- Names are the five operation names; the rule lines are the descriptions (`aria-describedby` on the tooltip content, also on focus).
- Non-text contrast: glyph 8.3:1 on `--toolbar-bg`; the dimmed (disabled) state is exempt but stays focusable.
- Hit target 28 x 28 px (the bar's size; the 2 px gap keeps neighbours apart).
- The two notices are live regions that exist in the tree from the start (empty), so the announcement happens on the text change.
- State is never colour alone: the open-path cause is text; the red outline is paired with the notice.
- No motion exists, so `prefers-reduced-motion` needs nothing.

### 10. Compound paths in the UI (criteria 31 to 38)

- **Properties panel subject line:** "Compound path", "3 compound paths" when every selected object is one, "N paths" when mixed with ordinary paths, "N objects" when mixed with primitives. The Style sections are those of a path. The Markers block is removed (hidden, not disabled) while a compound path is selected, until markers on compound paths are decided (default; the architect flagged `0018`).
- **Select bar** for a compound path: no kind group; "Object to path" is not shown (criterion 36); the Boolean group as above.
- **Node tool (criterion 38):** with a compound path as the only selected object the Node bar slot shows a text-only pill (the bar surface, no controls): "Nodes of compound paths cannot be edited yet." (`role="status"`, so a screen reader hears it on entry). The outline is drawn, no node, handle or segment overlay. **Double-click on a compound path in the Select tool** does not switch tools (the precedent of the primitives, where a double-click that does nothing gives a hint): the shared hint chip shows the same sentence for 3 s. Pressing `N` or the rail button still enters the Node tool and shows the pill.
- Click inside a hole picks what is behind (criterion 33): the hover box follows what a press would pick, so over a hole it is the object behind that lights up.

### Criteria changes requested

(All applied by the product owner on 2026-10-09, in criteria 2, 15 to 18, 22a, 29, 38, 47a and P1 to P3. Kept for the record.)

- **2:** "Tab" becomes "the group is one Tab stop with roving focus"; tooltips are three lines (name, rule, note) not "at most two lines"; the note text varies by state as in section 4.
- **15, 16:** add the red outline of the offending operands while the notice is shown (section 5); the notice stays 8 s or until the next action. Replace the example with the wording table.
- **17:** use the four "empty" sentences of section 5.
- **18:** add "the refusal notice is an alert; a mouse activation returns focus to the canvas, a keyboard activation keeps it on the button".
- **22 (new 22a):** after a successful operation focus is on the canvas.
- **29:** the sentence ends with "No undo yet."; "path" is "compound path" when the result is one.
- **38:** add the double-click hint chip and the bar-slot sentence above.
- **44 to 47 (new 47a):** from activation until the result is shown the window is busy (section 7).
- **P1:** hover intent 100 ms; keyboard focus counts only when it came from the keyboard; Escape removes it. **P3:** the tooltip note says "Would be empty" when the preview found it so.

### Open design questions (defaults taken)

1. Pre-dimming for open paths (section 3). Default: yes. Alternative: always enabled, refuse on click.
2. Red outline of offending operands (section 5). Default: yes. Alternative: counts only in the sentence.
3. Markers block on compound paths. Default: hidden. Decide with the markers owner.
4. Preview on keyboard focus (section 6). Default: yes, keyboard-initiated focus only.

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
