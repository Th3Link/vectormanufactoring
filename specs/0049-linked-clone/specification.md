# Linked clone: a second object that follows its original

Status: Draft (the criteria are complete and testable; becomes Ready when `adrs.md` is confirmed by the architect, the customer has accepted the amendment of ADR 0002 §5 it proposes, and the UX notes are confirmed by the ux-engineer, `CLAUDE.md` §4)
Priority: Should
Origin: Customer (request of 2026-10-10: a "linked clone" is a second origin object that contains all transformations of the original and changes live with it, and can still be moved, scaled, rotated and skewed on its own; only for paths; a rectangle or other primitive is converted to a path first, silently, as decided in `0047`). Everything in "Decided by the product owner" and every criterion marked **Proposal** is my own idea and is not accepted until the customer says so.

## User value

As a maker I want a clone of a path that stays linked to it, so that I can make twelve identical hinge tabs, mirror-placed parts or repeated holes, fix the shape once in the original, and have every clone change with it, while each clone still sits where I put it, turned and scaled as I need.

**What the other tools do, and what we do.**

- *Inkscape* (Edit > Clone, `<use>` in the file): Alt+D creates a clone exactly on top of the original, Shift+D selects the original, Unlink Clone turns it into a path. The clone shows the original's style unless the clone overrides a property. A preference decides what happens when the original is deleted (the clones are unlinked, the default, or deleted); a clone whose original is missing is an "orphaned clone". Alt+D on a clone makes a clone of the clone, a chain. A clone pasted into a file without its original is orphaned. A path operation on a clone unlinks it (preference). The clone can be moved, scaled, rotated and skewed; the original is untouched.
- *LightBurn*: the nearest thing is the Virtual Array, a grid of synced copies of one source. The copies cannot be selected or changed on their own, are drawn dashed, and are flattened into ordinary copies when the job is output.
- *Illustrator and Affinity*: symbol instances. The instances follow the symbol, can be transformed on their own, and "Break link to symbol" makes an ordinary object.

**What we do the same:** clone on top of the original, Select original, Unlink, unlink (not delete) when the original is deleted, output as plain paths.
**What we do differently:**

- *No chains.* A clone of a clone is a clone of the same original (decision 3). Nothing can form a cycle and finding the original is one step.
- *Every clone is a full object.* Select it, box it, transform it by gesture or typed value, group it, hide it (`0050`), exactly like a path. LightBurn's virtual copies cannot be touched.
- *No surprise when the original dies.* The clones become paths in the same step and stay where they are (criterion 31), where Inkscape needs a preference and leaves orphans.
- *The link is visible in the panel, inline.* The Style tab says "Linked clone of Path 3f9a1c" with Select original and Unlink buttons; the original says "3 linked clones" with Select clones. Inkscape shows it in the status bar, as far as I know.
- *Silent conversion of a primitive.* A rectangle is converted to a path as part of making the clone, with no warning (`0047`).

## Words used below

- **Origin:** the path or compound path that clones refer to. It is an ordinary object; it knows nothing of its clones.
- **Clone:** an object of its own kind that has no outline, style or anchors of its own. It stores the id of its origin and a **clone matrix** M. It is not a primitive, a path or a group.
- **Clone matrix M:** an affine map `x' = a·x + c·y + e`, `y' = b·x + d·y + f` (SVG order a, b, c, d, e, f), document millimetres, X right, Y down. Its linear part is L = (a, c; b, d). The identity is (1, 0, 0, 1, 0, 0).
- **Resolved outline of a clone:** the origin's outline (every outline of a compound path) with M applied: every anchor point p becomes M·p, every handle vector v becomes L·v, open or closed state and node order are kept. Curves are cubic Béziers, so this is the exact image. The clone is drawn, hit-tested, boxed and measured by its resolved outline.
- **Local frame:** the clone's own axes, M applied to the document axes. A change of the origin is seen by the clone in this frame (criterion 12).
- **Bake / unlink:** turn a clone into an independent path with the resolved outline (criterion 31).
- **Orphan:** a clone whose origin does not exist, is not a path or compound path, or is itself a clone.
- Lengths are compared with the document's geometric tolerance, angles with 1e-9 rad, as in the earlier specs.

## Acceptance criteria

### Making a clone

1. Given the Select tool and one or more selected objects, each a path, compound path, rectangle, ellipse, polygon, star or clone, when the maker activates **Linked clone**, then for each selected object one new clone is created, its matrix is the identity (it lies exactly on what it was made from), it sits directly above that object in the stacking order, and the new clones are the only selected objects. One commit, labelled `linked_clone`. Test: A, B, C selected, the order becomes A, A', B, B', C, C' and A', B', C' are selected.
2. Given a selected rectangle (with or without rounded corners, turned or not), ellipse, polygon or star, then it is converted to a path in the same commit, before the clone is made, with no warning, hint, notice text or confirmation about the conversion. The converted object is "Object to path" of that shape (`0003` criteria 17 to 22, `0013` criterion 16): same id, same place in the stacking order, same style, same nodes. The clone's origin is that object. Test: a rectangle with four different corner radii gives a path of 8 nodes and a clone whose resolved outline equals that path's.
3. Given a clone selected, then Linked clone makes a clone of the same origin with the same matrix (a clone of a clone is a clone of the origin, never a chain; decision 3). Test: C1 = clone of P with a matrix that is not the identity; Linked clone on C1 gives C2 with origin P, the same matrix and the same resolved outline; C2 sits above C1.
4. Given a selection that contains a group (`0023`) or a layer (`0050`), then Linked clone is not offered, and nothing is converted. Given no selection, or a tool other than Select, then the button is not shown (it lives in the Select bar, UX notes).
5. Given a successful Linked clone, then a one-line notice reads "Created 1 linked clone." or "Created 3 linked clones.", disappears within 3 seconds, takes no focus and is a `role="status"` live region. It never mentions a conversion. Until `0020-undo-redo` has merged it ends with "No undo yet." as in `0016` criterion 29; the build that merges second removes the sentence (`0020` criterion 52).
6. Given any state of the app, then no key starts Linked clone, Select original or Unlink clone; they are buttons only (default of Question 6). Test: the key table has no entry for them.

### What a clone shows and how it follows

7. Given a clone with matrix M, then its drawn, hit-tested and measured outline is its resolved outline. Test: an origin square of side 10 mm at (0, 0) and M = scale 2, translate (30, 0): the clone is the square from (30, 0) to (50, 20). A compound path origin (a ring) keeps its hole: the point that was the ring's centre, after M, is not painted (`0016` criterion 31).
8. Given a clone, then it is drawn with its origin's style, read live: fill on or off, fill colour and opacity, stroke on or off, colour, opacity, dash, join and cap, markers. The clone has no style of its own (decision 2). Test: change the origin's stroke colour to red; the clone is red in the next frame.
9. Given M with a scale, then stroke width, dash lengths and marker size of the clone are not scaled by M: they are the origin's values in document millimetres. Test: origin stroke 0.5 mm, M = scale 4: the clone's stroke is 0.5 mm wide on the canvas (a laser hairline stays a hairline). The "Scale stroke width" and "Scale corner radius" switches do not apply to a clone.
10. Given any edit of the origin that keeps the origin object (a node, handle or segment drag, add, delete or change of a node, Join and Split of an open path keeping this object, a move, resize, rotate or skew, a style edit), then after the commit every clone's resolved outline is M applied to the origin's new outline, in the next frame. Test: origin square, clone with M = translate (100, 0), the origin's top-right node is dragged by (5, 5): the clone's top-right node moves to the origin's new node plus (100, 0).
11. Given a drag that edits the origin (a node, handle or segment drag, a move, resize, rotate or skew), then during the drag each clone is previewed like a selected object: its committed outline stays in its own style and the outline the release would give is drawn as the 1.5 px blue outline (`unified-object-editing` criteria 10 to 14). With more than 500 clones of the origin, no clone preview is drawn and the clones jump on release. Nothing is written before the release.
12. Given the origin changes by a map T (a move, a rotation, a scale), then the clone's outline changes by M·T·M⁻¹ in the document, that is, the clone sees the same change in its local frame (resolved' = M·T·origin; for a translation t of the origin the clone moves by L·t). Test: M = turn by 90° clockwise on screen, (a, b, c, d) = (0, 1, -1, 0); the origin is moved 5 mm right; the clone moves 5 mm down. A clone with M = translate only moves 5 mm right.
13. Given an edit of the origin by one peer and a transform of the clone by another peer at the same time, then after the merge the clone's resolved outline is the clone's new matrix applied to the origin's new outline (two registers, no conflict). Given two peers that transform the same clone at the same time, then after the merge the matrix is one of the two matrices as a whole, never a mix of their entries (the matrix is one atomic value, decision 1).

### Transforming a clone

14. Given the Select tool and a selection that holds only clones, then it has the group box and the handles of `0019` over the resolved outlines: move, scale by corner and edge, rotate, skew, centre handle, typed entries (M, R, S, K) and the typed move of `0047` with its reference points. Skew and stretch are offered for every clone and never convert anything; no conversion notice appears. The box is the tight axis-parallel bounds of the resolved outline; it does not remember a rotation (`0019` criterion 7). A clone selected together with other objects uses the group box of the whole selection.
15. Given a transform gesture whose map in the document axes is G (a move, scale, rotate or skew, drag or typed) on a selection that contains a clone and not its origin, then the clone's matrix becomes G·M, the origin is not written, and the clone's drawn outline is G applied to the outline it had (within 1e-6 mm). One commit, the label of the other transforms (`transform_objects`).
16. Given the same gesture on a selection that contains a clone together with its origin, then the origin is transformed as an object (its geometry becomes G applied to it) and the clone's matrix becomes G·M·G⁻¹, so that the clone's drawn outline is G applied to the outline it had (within 1e-6 mm): the clone keeps its place relative to the origin, as in any multi-object transform (`0019`). Given G has a determinant with absolute value below 1e-9 (a factor of 0 on one axis), then the clones of that case are unlinked first (criterion 31) and then transformed as paths. Test: the origin square at (0, 0), the clone at (30, 0) with M = translate (30, 0); both selected and scaled by (2, 1) about (0, 0): the origin is 20 wide, the clone covers x = 60 to 80.
17. Given a transform that would put any coordinate of any resolved outline beyond ±1e7 mm, or any entry of a matrix beyond ±1e7, or make a value not finite, then nothing is written and nothing is converted, all or nothing, as `0019` criterion 22 says.
18. Given the Select bar with only clones selected, then the two switches "Scale stroke width" and "Scale corner radius" are not shown; with a mix they act on the selected objects that are not clones.
19. Given Duplicate or a Ctrl-copy move (`0010`) of a clone, then the copy is a clone of the same origin with the same matrix (displaced by the move for a Ctrl-copy), above its source. Given Duplicate or a Ctrl-copy move of an origin, then the copy is an independent path that no clone refers to (the existing copy rule, `0010` adrs.md decision 2); the original's clones stay linked to the original.

### Selecting, the Node tool and the panel

20. Given a click, marquee, lasso or Alt-click cycle (`0014`), then a clone is hit by its resolved outline and its filled interior (holes of a compound origin are not hit) and is one candidate like any object.
21. Given one clone selected, then the Properties panel subject line reads "Clone of Path 3f9a1c" (the origin's name as the object list shows it: kind and the six-character id, or its name once `0050` exists); given several clones "3 clones"; given a mix of kinds "N objects". The slot is 120 px wide (80 px once `0050` adds its tab), so the text is cut with an ellipsis there; the tooltip and the first line of the Style tab block (criterion 22) give the name whole.
22. Given a selection that holds only clones, then the Style tab shows no style section and no marker block. In their place it shows an inline block, no popup: the text "Linked clone of Path 3f9a1c. It shows the style of its original." and two buttons, **Select original** and **Unlink clone**; for several clones "3 linked clones." with **Select originals** and **Unlink clones**. The block is in the tree only for such a selection (hidden, not disabled).
23. Given a selection that holds an origin (and no clone), then the Style tab shows its sections as for any path and, above them, the line "2 linked clones follow this object." with the button **Select clones** (the line is in the tree only while the origin has at least one clone). Given a selection of an origin and some of its clones, then both lines appear for what applies.
24. Given a style edit on a selection that holds clones and other objects, then the edit is written to the other objects only; the clones follow their origins, and the "Mixed" values of `0017` criteria 9 and 14 are computed from the objects that are not clones. A clone is never given a style.
25. Given **Select original** with clones selected, then the selection becomes their origins (each once). Given **Select clones** with origins selected, then the selection becomes all clones of them. Both are view state: no commit. The origin that is in another group is selected in that group's context (`0023` criterion 15). An origin or clone that is effectively hidden or locked cannot be selected (`0050` criterion 14): it is left out; when none of the targets can be selected the selection stays as it was and a refusal notice says so ("The original is hidden or locked. Show or unlock it in the Layers tab. Nothing was changed."), when some can the notice reads "Selected 2 of 3 originals. The rest are hidden or locked." (UX notes). While the pointer or the keyboard focus is on Select original, the hover box of `0014` is drawn on each origin (at most 50) and removed in the frame the pointer or focus leaves (Proposal).
26. Given the Node tool and a selection of one clone, then the resolved outline is drawn, no node, handle or segment overlay is shown, and the Node bar slot shows a text-only line "Linked clones have no nodes. Edit the original or unlink the clone." (`role="status"`). Given a double-click on a clone with the Select tool, then the tool does not change and a hint chip shows the same sentence for 3 seconds. A clone is no target of Join, Split, segment bending (`0031`) or the Pen's continue and connect (`0034`); those refuse or ignore it and nothing changes.
27. Given "Object to path", then it is not offered for a clone (Unlink clone is its counterpart).

### Unlink, delete and the other commands

28. Given one or more clones selected, when the maker activates **Unlink clone**, then each clone becomes an independent path (a compound path when the origin is one): the same id, the same place in the stacking order, still selected; the anchors are the resolved outline's with new anchor ids that are unique in the document (the caller mints them); the style is a copy of the origin's style; `rotation` 0. The drawn outline before and after is the same (every point within 1e-9 mm). One commit, `unlink_clone`; the origin and the other clones are unchanged. A one-line notice reads "Unlinked 1 clone." or "Unlinked 3 clones." (status, 3 seconds).
29. Given an unlinked path, then a later edit of the former origin changes nothing in it, and the Node tool edits it like any path.
30. Given a clone is deleted, then only the clone is removed. Given Delete on a selection, then the whole selection is removed.
31. Given an origin is removed by any command, that is Delete, a Boolean operation, Combine, Break apart, Split, Fracture, Flatten (the commands of `0016`, `0035`, `0036`, `0037`, `0048`) or the last-node delete of the Node tool, then in the same commit every clone of it that is not itself removed by the same command is unlinked (criterion 28 with the origin's outline as it was just before). Nothing disappears: the clones stay where they were, as paths. The commit is atomic. A one-line notice adds the count: "Deleted 1 object. 3 clones became paths." / "Union: 3 objects became 1 path. 2 clones became paths." (status, 3 seconds). Test: origin P with clones C1, C2; Delete P: C1 and C2 are paths with P's outline under their matrices and P is gone; Delete P and C1 together: only C2 becomes a path.
32. Given a command that keeps the origin object and its id (Duplicate, which copies; a style edit; a move, resize, rotate or skew; Offset, which keeps the original), then the clones stay linked. Given a command that replaces the origin by another object with a new id, then criterion 31 applies.
33. Given a command that reads the geometry of selected paths (Booleans, Combine, Break apart, Split, Fracture, Flatten, Offset, Split compound path), then it reads a clone's resolved outline as if it were that path, and a clone among the operands is consumed like any operand. Given a result that takes the base operand's style (`0016` criterion 21) and the base operand is a clone, then the style is the origin's. Test: Union of a clone with M = translate (20, 0) and a path that overlaps it gives the same outline as the union of the unlinked clone and the path.
34. Given Group and Ungroup (`0023`), then a clone and its origin may be in different groups or layers and the link holds (it is by id); no clone changes how it looks. Given a group transform (`0023` criterion 22) of a group that holds a clone and its origin, criterion 16 applies; of a group that holds only the clone, criterion 15; of a group that holds only the origin, the clone follows (criterion 10).
35. Given a document that holds orphans after Open or after a merge (a peer deleted the origin while another created a clone of it, or a file written by another tool), then an orphan is not drawn, not hit-tested, not selected and not output, and the editor removes all orphans in one repair commit (`remove_orphan_clones`) right after the Open or the merge, with the notice "Removed 2 clones whose original was deleted." The project is marked changed. Given a clone whose origin is itself, or two clones that refer to each other, then both are orphans and are removed the same way; nothing loops or crashes.
36. Given `0020` has merged, then Linked clone, Unlink clone, the orphan repair and every command of criterion 31 are one step each with the operation names "Linked clone", "Unlink clone", "Remove orphan clones" (the guard of `0020` criterion 41 fails the build without these entries). Taking back a Delete or Boolean that unlinked clones restores the origin with its id and turns those clones back into clones with their matrices (the kind of the node changes back, field by field, the engine of ADR 0014 §1). Redo repeats it.

### Saving, the file and hostile input

37. Given a project with clones is saved and reopened, then every clone has the same origin id, the same matrix to the last bit, and the same place in the stacking order; every origin and every clone draws as before closing. `format_version` goes to the next free number at merge (`specs/README.md`, "`format_version` plan"); a file with a clone opened by an earlier build is refused with the "saved by a newer version" message (an earlier build would read a node without an outline); a file from an earlier build opens unchanged and is not rewritten on open. A golden file `clone_v<N>.curvyo` pins the format.
38. Given a file whose clone has no origin id, an origin id that is not a valid id string, no matrix, a matrix that is not a list of exactly six numbers, or a matrix with an entry that is not finite or lies beyond ±1e7, then the file is refused as damaged and the editor does not crash. A clone whose origin id is valid but names nothing, a non-path or another clone is not damaged: it is an orphan (criterion 35).
39. Given a file with 100,000 clones of one origin, or with clones that refer to each other in a ring of 10,000, then it opens without hang or crash (the ring becomes 10,000 orphans, removed by the repair commit).
40. Given the `document.json` view, then a clone is listed with `"shape": "clone"`, the origin's id and the matrix, and no outline. It is a non-authoritative view (ADR 0004 §1).

### Output

41. Given any output of the project (SVG export, a job, a preview), then a clone is never an output kind: it is handed over as a path whose outline is the resolved outline and whose style is the origin's. Test in the document core: the path read of a clone equals the path read of the same clone after Unlink clone (anchors within 1e-9 mm, style equal). Hidden clones follow `0050`.
42. Given the SVG export (`0024`, not yet specified), then a clone is written as an ordinary `<path>` with the resolved outline (decision 5, Question 7). This spec states the requirement; `0024` carries the test.

### Performance

Measured on the CI runner, release build, native; the browser build may take up to twice as long.

43. Given a project with 10,000 clones of one path of 100 nodes, then reading all objects with the clones resolved takes at most 300 ms, the first frame after opening is drawn within 1 s, and a frame in which the origin is changed (one node dragged, previews off) takes at most 1.2 times a frame of 10,000 ordinary copies of that path.
44. Given an origin with 10,000 clones and one node edit committed, then the next frame is drawn within 150 ms after the commit.
45. Given 10,000 selected paths, then Linked clone, including the commit, finishes within 5 s (`0019` criterion 49), with the wait cursor and the busy state of `0016` criterion 47a; Unlink clone of 10,000 clones the same.
46. Given 10,000 clones, then the saved file is no more than a few hundred bytes per clone larger than the same file without them; the added size is measured and reported in the PR description, not gated.

## Decided by the product owner

Change any of these if you disagree. The ones that could cost the customer something later are repeated as questions below.

1. **Model.** A clone is its own kind of node: `origin` (an object id) and `matrix` (one atomic list of six numbers). It has no outline, anchors, style or rotation of its own. The resolved outline is computed when the document is read, so everything that draws, hits, measures or outputs a path sees a clone as a path. One atomic matrix register, because two peers writing single entries would mix into a matrix neither of them made.
2. **Style follows the origin; there are no style overrides.** Inkscape lets a clone override single properties. Here the only thing that differs between a clone and its origin is the matrix and, once `0050` exists, the per-object opacity, visibility, lock and name of the clone. Reason: a maker who needs a differently coloured copy for another machine role wants an independent path, and Unlink clone is one click; overrides need a way to show, edit and reset per property, which is a panel of its own. ADR 0002 §5 says "no inheritance"; a clone's style is a reference to one other node's style, not a cascade, and this is the one exception (flagged to the architect).
3. **No chains.** A clone of a clone is a clone of the origin. A path never becomes a clone, so no cycle can form and every clone has one hop to its origin.
4. **A clone is placed exactly on its origin**, as Inkscape and Affinity do (Question 4 for an offset).
5. **Output expands.** A clone is a path in every output. SVG `<use>` round trip is not part of this slice (Question 7).
6. **The matrix is applied on top of the origin's stored geometry.** Because every transform in this product is baked into anchors and frames, the origin's stored geometry already holds "all its transformations"; the matrix adds the clone's own. No transform is stored on the origin.
7. **A hidden or locked origin does not hide or lock its clones** (`0050`): the clone has its own flags. A hidden original with visible clones is a normal state, and the way to keep a master out of the way.

## Out of scope

- **Clones of groups, layers, text, images or primitives as such.** Only paths and compound paths are origins; a primitive is converted first (criterion 2). Clones of groups are a later entry.
- **Style overrides on a clone**, and any per-property link to the origin (decision 2).
- **Editing nodes through a clone** (Question 5), and any command that needs the clone's nodes (Join, Split, segment bending, Pen extension).
- **Tiled or arrayed clones** (Inkscape's Create Tiled Clones, LightBurn's Grid Array): a grid or circle of linked clones from one dialog. A natural follow-up for makers; a Proposal for a later entry, not part of this slice.
- **Relink**: pointing a clone to another original ("Relink to copied" in Inkscape), turning a path into a clone, and clones that follow a clone.
- **SVG import and export** of `<use>` (`0024`, Question 7).
- **Copy and paste between projects.** No object clipboard exists. When one does, a clone pasted into a project that lacks its origin must paste as an independent path (Inkscape pastes an orphan). Recorded for that spec.
- **Keyboard shortcuts** for the three commands (Question 6).
- **Undo and redo** themselves (`0020`); this spec only fixes the steps (criterion 36).
- **Anything a job does with clones**; a clone is a path to it (criterion 41).

## Open questions

Each has a default; nothing blocks except the document-model ADR.

1. **Document model (criteria 37, 38; decision 1).** A new node kind, two registers, a `format_version` bump, one exception to "no style inheritance". `needs-customer`, as for every document-model change (`CLAUDE.md` §3). Recommendation: no new ADR number, as for the compound path and the group; one dated amendment of ADR 0002 §5 that names the exception, shared with `0050` (see `adrs.md`). Default: yes. The alternative is no linked clones.
2. **Style overrides (decision 2).** *A (default):* none; the clone shows the origin's style. *B:* overrides per property (colour, stroke width), with a panel that shows and resets them. Recommendation A: a separate, larger spec if it turns out to be needed.
3. **Clone of a clone (criterion 3).** *A (default):* flattened to a clone of the origin. *B:* a real chain, as Inkscape (a clone follows a clone's matrix too). B needs cycle checks on every merge and repair; recommendation A.
4. **Where the clone appears (criterion 1).** *A (default):* exactly on the origin. *B:* displaced by 10 mm right and down, so the maker sees at once that something happened. With A the notice and the selection box are the cue.
5. **Node tool on a clone (criterion 26).** *A (default):* no nodes; the hint tells the way out. *B:* the Node tool edits the origin's nodes through the clone (the pointer mapped back through M⁻¹), so the maker never has to find the original. B fails for a matrix with determinant 0 and confuses when the origin is elsewhere on the page. Recommendation A.
6. **Shortcuts.** *A (default):* none. *B:* Inkscape's Alt+D (clone), Shift+D (select original), Shift+Alt+D (unlink). Alt+D focuses the address bar in a browser, so B would be wrong for the browser build; recommendation A until a key is found that works everywhere (the key table is `docs/design-system.md`).
7. **SVG (criterion 42).** *A (default):* a clone is written and read as a plain path. *B:* a clone is written as `<use xlink:href="#id" transform="matrix(...)">` so Inkscape sees a clone, and `<use>` of a path is read as a clone. B is correct only when the matrix has no scale, because SVG scales a clone's stroke and ours does not; so B writes `<use>` only for a rotation and translation and a path otherwise. Decided in `0024`; the default here is A.
8. **Where the buttons live.** Default in the UX notes: Linked clone in the Select bar, last, after "Offset"; Select original, Unlink clone and Select clones in the Style tab block (criteria 22, 23) and, once `0050` exists, as row actions. Not the rail: its two columns are allocated (`docs/design-system.md`, "Tool rail architecture"), and a further family of commands goes to the Select bar or the panel.

## UX notes

Confirmed and completed by `ux-engineer`, 2026-10-10. The product owner's draft stands except where a line says "Changed". Numbers and rows are in `docs/design-system.md`: "Linked clone in the UI", "Select bar layout", "Action notice", "Properties panel: Layers tab". References (`docs/reference-tools.md`): Inkscape's Clone menu (Alt+D, Shift+D, Unlink), Illustrator and Affinity symbol instances, LightBurn's Virtual Array.

### The button in the Select bar

- **Linked clone** is a text button in the Select bar, the look of "Offset" (28 px, 1 px `--toolbar-icon` outline at 60 %, `rounded-[5px]`, 14 px label). **Changed: no `Link2` glyph, and it comes last, after Offset.** The bar already holds the Link corners toggle (`Link`, `Unlink`); a second near-identical link glyph with another meaning, in the same bar, is a trap. The bar's rule is that a new control is appended so that no earlier control moves ("Offset" is last for that reason), so Linked clone follows Offset in a group of its own.
- Shown (in the tree, never disabled) when the selection holds at least one object and no group. **Changed:** the draft said "at least one clonable object", which would offer the button for a group together with a path, against criterion 4. A layer is never in the selection (`0050`), so criterion 4 needs no layer case. Tool other than Select: the bar is not there.
- Tooltip, `side="bottom"`, three lines: "Linked clone" / "A second copy that follows the original." / "Edit the original and every clone changes."
- **Criterion 18 is an exception to the bar's rule that the two switches are "always shown".** With only clones selected "Scale stroke width" and "Scale corner radius" leave the tree, because they would do nothing, and an inert switch would lie. The first row then begins with Offset and Linked clone. The switches come back with any other object.
- Not on the rail: column B is full and column A has room for two tools (row "Tool rail architecture"); a new family of commands goes to the Select bar or the panel. This holds together with the rail layout that `0048` fixed (Path card first in column B, Boolean card under it).

### The Style tab

Top to bottom: the Offset section when open (a mode the maker entered), the clone block, the origin line, then the Style sections. All inline, no popup.

- **Clone block** (only clones selected; `role="group"` named "Linked clone"). Row 1, 14 px `--toolbar-icon`: `Link2` 16 px (`aria-hidden`), 4 px, "Linked clone of Path 3f9a1c." (the draft's sentence, split in two lines of the same text). Row 2, 12 px `--panel-muted-fg`: "It shows the style of its original." Several: "3 linked clones." and "They show the style of their originals." A name of the original longer than 40 characters is cut with an ellipsis, the tooltip has it whole. Then 8 px and two full-width (244 x 28 px) outline buttons of the "Fit to content" look, stacked 8 px apart: **Select original**, **Unlink clone** (plural labels "Select originals", "Unlink clones"; the 118 px side-by-side layout does not fit "Select originals"). Height about 112 px (up to 132 px when row 1 wraps). **Changed: the draft said about 90 px.** It scrolls with the body.
- **Origin line** (an origin selected and at least one clone exists): `Link2` 16 px and 12 px `--panel-muted-fg`, "2 linked clones follow this object." (one: "1 linked clone follows this object."; several origins: "5 linked clones follow these objects."), the full-width outline button **Select clones** ("Select clone" for one) 8 px under it. About 52 px. The count is live (a peer's clone, an unlink). With an origin and its clone both selected, both blocks show, clone block first.
- **Tooltips** (text only, `side="left"`, 400 ms): Select original "Selects the object this clone follows." Unlink clone "Turns the clone into an ordinary path. Shape and style stay." Select clones "Selects every clone of this object."
- **Hover cue** (criterion 25, Proposal, confirmed): the `--hover-box` on each original while the pointer or focus is on Select original, at most 50, removed in the frame it leaves. This is the only canvas cue for the link. The same box on the clones for Select clones is not drawn (there may be thousands).
- **Select original / Select clones with hidden or locked targets** (`0050` criterion 14: the selection never holds them). Targets that cannot be selected are left out. If none can be selected the selection stays and a refusal notice (`role="alert"`, 8 s, 4 px under the button) says "The original is hidden or locked. Show or unlock it in the Layers tab. Nothing was changed." ("The originals are...", "The clone is...", "The clones are..."); if some can, a status notice says "Selected 2 of 3 originals. The rest are hidden or locked." A hidden master with visible clones is a normal state (decision 7); the way to edit it is to show it first.
- **Focus.** Mouse press: focus returns to the canvas (`0016` criterion 22a). Key press: Unlink clone removes its own block, so focus goes to the first control of the Style area (Stroke Paint); Select original shows the origin's body, so focus goes to its first control, which is the Select clones button. The notice of Unlink clone ("Unlinked 3 clones.", 3 s, `role="status"`) is anchored where the button was (4 px below its last position, right-aligned), as a child of the panel body and not of the vanished block. Linked clone's notice sits 4 px under the Linked clone button. The notices of criteria 31 and 35 go where the command that caused them posts: the canvas notice slot for the Delete key and the orphan repair, beside the rail for a rail button.

### On the canvas

- **A clone looks like the path it follows**: the origin's fill, stroke, dash and markers, the stroke width in document millimetres whatever M scales (criterion 9), holes kept. Nothing marks it when idle: a cue on every clone would put noise on artwork that is also the production picture (twelve hinge tabs must look like twelve hinge tabs), and Illustrator, Affinity and Inkscape mark nothing either, as far as I know. The link is shown on demand (the Style block, the hover cue above, the row in the Layers list). Open to the customer: a faint link mark on selected clones; default none.
- **Selected clone**: the usual dashed blue box and the handles of `0019` over the resolved outline; no member-box difference. Move, scale, rotate, skew, the typed move (`M`) with its reference points (`0047`), `R`, `S`, `K` work as for a path; `K` never refuses a clone.
- **During a drag that edits the origin** each clone is previewed as a selected object is (1.5 px `--preview-new` outline, criterion 11), none above 500 clones.
- **Node tool**: one clone selected: the resolved outline, no node, handle or segment overlay, and the Node bar slot holds a text-only pill (bar surface, 14 px `--toolbar-icon`, `role="status"`) "Linked clones have no nodes. Edit the original or unlink the clone." (as the compound path pill). The pill shows only when every selected object is a clone or compound path; with a path and a clone selected the path's nodes show and the clone draws its outline, as `0016` criterion 38 does for compound paths. The double-click hint chip (3 s) says the same.
- **Orphan, unlink and delete** are never seen as states: an orphan is removed on load, an unlink or delete of an origin turns the clones into paths in the same frame.

### The Layers list (`0050`)

A clone row has `Link2` as its kind glyph (16 px), the name "Clone of <origin name>" (the origin's own name, or "Path 3f9a1c" when it has none), its own eye, lock and opacity, and the row action **Select original** (`Locate`, 24 px) in the row-action slot. The origin's row looks like any path. Accessible name "Clone of Path 3f9a1c, linked clone, hidden, opacity 60 percent". Renaming a clone sets the clone's name; the origin's name is not touched.

### Keyboard, screen reader, contrast, touch, 800 x 600

- The block's buttons and Select clones are ordinary Tab stops in the body after the strip, in the order Select original, Unlink clone; Enter and Space press; names are their texts. Linked clone is a Tab stop in the Select bar after Offset. No key starts any of the three (criterion 6).
- Notices are `role="status"` (refusals `role="alert"`) and never take focus. The block is a `role="group"`; the glyphs are `aria-hidden`; nothing is told by colour alone (a text sentence names the link).
- Contrast: the block uses `--toolbar-icon` (8.3:1) and `--panel-muted-fg` (5.0:1) on the panel; the buttons are the existing outline buttons (border 3.1:1, label 8.3:1).
- Touch: the buttons are 28 px high at full width and need no hover; the hover cue is a pointer extra. 800 x 600: nothing is added to the canvas; the Style tab gains the block (112 px) or the origin line (52 px) and scrolls as it always does; the Select bar gains one 112 px button that wraps with its group.

### Fit with the neighbours

- **`0043` tabs.** The block is in the Style tab; Layers stays active when a clone is selected there (`0050` criterion 2). Nothing about tabs changes. A maker who works in Layers reaches the link through the row action; the Style tab (Shift+Ctrl+F) has the full block.
- **`0044` Ctrl+A and nudge.** Clones are objects: Ctrl+A selects them, the arrows nudge them (a clone's matrix changes, the origin does not).
- **`0023` groups.** A group in the selection hides the button (criterion 4); a clone in a group is cloned inside that group's context; a clone and its origin in different groups keep the link (criterion 34); Select original in a group enters it (criterion 25).
- **`0047` transform.** No conversion and no notice for a clone's skew or stretch; the silent conversion applies once, when the clone is made from a primitive (criterion 2); the typed move works on the clone's resolved box.
- **`0048` rail.** Nothing is added to the rail. Split, Break apart, Cut and the Boolean card read a clone as its path (criterion 33); their notices are unchanged and add the count of clones that became paths (criterion 31).

## Links

Requirements: R-EDIT-030 (`docs/requirements.md`); related R-EDIT-004, R-EDIT-012
Builds on: `specs/0019-multi-object-transform/` (group box, handles, atomic commit, limits), `specs/0047-transform-polish/` (silent conversion, typed move reference), `specs/0003-primitive-shapes/` (Object to path), `specs/0016-boolean-operations/` (compound paths, notice style, busy state), `specs/0014-advanced-selection/` (hit rules, hover box), `specs/0017-style-panel-rework/` (Style tab, Mixed)
Amends: the geometry-reading commands `0016`, `0035`, `0036`, `0037`, `0038`, `0048` (they read a clone's resolved outline, criterion 33), `0034`, `0031`, `0006` (a clone is no target, criterion 26)
Related: `specs/0023-groups/` and `specs/0050-layers-and-objects-panel/` (same ADR; flags of a clone, rows), `specs/0020-undo-redo/` (steps, criterion 36), `specs/0041-object-history/` (its "clone" is a copy of an object's history with the key `clone_of`; unrelated to a linked clone, the keys here are `clone_origin` and `clone_matrix`), `specs/0024` (SVG, not yet specified)
ADRs: `adrs.md` (architect; document model, `needs-customer`)
PR: TBD
