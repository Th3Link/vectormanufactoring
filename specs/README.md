# Feature specs

The list of all features comes first, then the convention for writing them.
State of this list: 2026-10-10 (the customer accepted ADR 0014 on 2026-10-10, which made 0020, 0041 and 0042 Ready; the customer renumbered every folder on
2026-10-09, see "Numbering" below; the customer requests of 2026-10-09 added
0030 to 0039 and gave the groups half of 0023 its spec; 0040 was added for the
customer request of 2026-10-10; the customer's undo and history specification of
2026-10-10 gave 0020 its folder and added 0041 to 0044, and his request for
customisable document formats and shapes of the same day added 0045 and 0046; his three change requests on silent conversion, sliders and the typed move's reference point added 0047).

Spec-driven development, adapted from
[spec-driven-dev-kit](https://github.com/trojava/spec-driven-dev-kit) for
this project's roles and file layout (see `CLAUDE.md` §2, §4).

## Feature list

One numbered list. The number is the folder's position in this list:
`specs/<NNNN-slug>/`. The list is grouped by state, and the numbers run
without gaps from top to bottom: first what is built (in the order it was
merged), then what is specified and waiting (in the order the lead starts it),
then drafts, then MVP slices nobody has specified yet. Rows without a link
have no folder yet; their number is reserved. **From 0030 on, numbers are
assigned in the order the folders are created** and a row sits in the group of
its state, so inside a group the numbers can be out of order (0023 and 0030 to
0038 sit in the "specified and waiting" group; the order the lead starts them
is in "Proposed build order" below).

Columns: **No.** = folder number. **MVP slice** = the label the same feature
had in the old MVP sequence (slices 1 to 16), `-` for features that came from
customer feedback and were never an MVP slice. Spec numbers and slice labels
agree for 0001 to 0007 only; see "Spec number versus slice label" below.

### Done

| No. | Slug | What it delivers | Status | Priority | Requirements | MVP slice |
|---|---|---|---|---|---|---|
| [0001](0001-project-file-foundation/) | `project-file-foundation` | Create, open and save a local `.curvyo` project with an empty canvas; same result after close and reopen. | Done (#3) | Must | R-SYS-001, R-SYS-002, R-SYS-007 | 1 |
| [0002](0002-path-node-editing/) | `path-node-editing` | Draw and edit a Bézier path with a pen tool: add, move and delete nodes, drag handles; same node, handle and segment model as Inkscape. | Done (#7) | Must | R-EDIT-001 | 2 |
| [0003](0003-primitive-shapes/) | `primitive-shapes` | Rectangle (with corner radius), circle/ellipse and polygon/star tools; "object to path" turns any of them into an editable path. | Done (#10) | Must | R-EDIT-002, R-EDIT-004 | 3 |
| [0004](0004-canvas-navigation-and-selection/) | `canvas-navigation-and-selection` | Pan and zoom the canvas (zoom toward the cursor), stable on window resize; a general Select tool to click, move and delete any object. | Done (#25) | Must | R-EDIT-010, R-EDIT-011 | 4 |
| [0005](0005-object-transform/) | `object-transform` | Move, scale and rotate any path or primitive with on-canvas handles on the Select tool's bounding box; "Scale stroke width" and "Scale corner radius" switches (both off by default). | Done (#29) | Must | R-EDIT-012 | 5 |
| [0006](0006-path-merge-split-and-node-types/) | `path-merge-split-and-node-types` | Join two path endpoints into one node, split a path at a node; a third node type (Asymmetric) next to Corner and Symmetric. | Done (#26) | Must | R-EDIT-014, R-EDIT-001 | 6 |
| [0007](0007-stroke-and-fill-styling/) | `stroke-and-fill-styling` | Stroke width, dash, join, cap and colour, and fill, on any path or primitive. The gradient fill and the popover panel built here are replaced by 0017 (customer decision after trying it, 2026-10-08). | Done (#54, #55, #58, #59) | Must | R-EDIT-005, R-EDIT-006 | 7 |
| [0008](0008-object-transform-refinements/) | `object-transform-refinements` | Centre move, rotate corners, pivots, typed entry, 22.5° snap, skew. | Done (#35) | Must | R-EDIT-012 | - |
| [0009](0009-unified-object-editing/) | `unified-object-editing` | One Select tool with parameter handles for every object; blue/black preview; shape tools only create. | Done (#38, fix #39, docs #40) | Must | R-EDIT-002, R-EDIT-011, R-EDIT-012 | - |
| [0010](0010-edit-interaction-polish/) | `edit-interaction-polish` | Polygon/star angle, Escape cascade, keyboard shortcuts, dashed selection box, typed skew and move, Shift axis lock, Ctrl copy of a move. | Done (#42, #45, #46) | Must | R-EDIT-012, R-EDIT-014 | - |
| [0011](0011-shape-creation-from-center/) | `shape-creation-from-center` | Shift while drawing a shape creates it from the centre. | Done (#47) | Must | R-EDIT-002 | - |
| [0012](0012-polygon-star-box-refit/) | `polygon-star-box-refit` | The selection box of a polygon or star follows the shape's shown orientation. | Done (#49) | Should | R-EDIT-012 | - |
| [0013](0013-rectangle-corner-radii/) | `rectangle-corner-radii` | A separate radius for each corner of a rectangle. | Done (#53) | Should | R-EDIT-002 | - |
| [0014](0014-advanced-selection/) | `advanced-selection` | Bigger hit area, candidate disambiguation, marquee and lasso select. | Done (#61) | Must | R-EDIT-010, R-EDIT-011 | - |

### Ready or in specification (next, in the order the lead starts them)

0015 and 0016 start first and run in parallel. 0017 follows. 0018 builds on
0017. 0019 runs after 0017 too: both change `ui-core` and `editor-wasm`, so
they do not run side by side. 0023 and 0030 to 0038 were added on 2026-10-10
from the customer's requests of 2026-10-09; their criteria are complete. 0030,
0031, 0034 and 0035 are `Ready` (`adrs.md` and UX notes exist); the others are
`Draft` until `adrs.md` and the UX notes come. 0020 and 0041 to 0046 were
specified on 2026-10-10 from the customer's texts on undo and history, on
the panel tabs, and on customisable document formats and shapes; their
`adrs.md` and UX notes exist since the same day. 0043 and the rectangular part
of 0045 are in progress in #84, 0044 in #87. 0020, 0041 and 0042 are `Ready`
since 2026-10-10: the customer accepted ADR 0014
(`docs/adr/0014-history-undo-and-branches.md`) and every default of the three
specs. 0046 stays a `Draft` sketch.
Their order, dependencies and conflicts are in "Proposed build order" below.

| No. | Slug | What it delivers | Status | Priority | Requirements | MVP slice |
|---|---|---|---|---|---|---|
| [0015](0015-document-size-and-rulers/) | `document-size-and-rulers` | Rulers in mm along the top and left, document resize with the content staying centred, fit the document to the drawing, drawing outside the document edge. | Done (#67, #72) | Must | none yet | - |
| [0016](0016-boolean-operations/) | `boolean-operations` | Union, difference, intersection, exclusion and reverse difference on closed paths, as a command section in the left tool rail; compound-path results. One PR (#73), built as milestones on one branch. The hover preview of the result is not part of it (see "Specified but not built" below). Amended 2026-10-10: the Boolean operations become their own toolbox card (small `fix/` PR). | Done (#69, #73) | Must | R-EDIT-003 | 9 |
| [0017](0017-style-panel-rework/) | `style-panel-rework` | Empty panel when nothing is selected, controls hidden instead of disabled, 8-digit RGBA hex, inline colour picker, eyedropper, custom dash text line, GIMP-style value fields, gradients removed. Replaces parts of 0007. | In progress (#78) | Must | R-EDIT-005, R-EDIT-006 | - |
| [0018](0018-stroke-markers/) | `stroke-markers` | Arrow or dot at the start, the end, N places along and on every node of a path. Builds on 0017. | In progress (#78) | Should | R-EDIT-005, R-EDIT-016 | - |
| [0019](0019-multi-object-transform/) | `multi-object-transform` | One group box with the same handles as a single object, for a selection of several objects: move, scale, rotate, skew (paths). Any selection can be stretched in one direction (edge handles, typed size); polygons, stars and turned shapes become paths in the same commit, with a notice; their corner drag stays proportional (customer change 2026-10-10). Runs after 0017; 0014 is merged. | In progress | Should | R-EDIT-012 | - |
| [0020](0020-undo-redo/) | `undo-redo` | Per-user undo (Ctrl+Z) and redo (Ctrl+Shift+Z) for every editing operation, one interaction = one step, peer-scoped in a shared document; every step has a short id, an author, a time and an operation name; the global history of the document is saved in the file and listed in a History tab (deletions visible); wipe of the document history; the "No undo yet" texts go. Specified from the customer's text of 2026-10-10. Needs `0043` for the History tab (milestone 3). | Ready (2026-10-10: ADR 0014 accepted with all defaults; `adrs.md` and UX notes exist; milestone 3 needs 0043, merged in #84) | Must | R-EDIT-008, R-HIST-001, R-HIST-004 | 8 |
| [0023](0023-groups/) | `groups` | Group and ungroup, nested groups, enter a group to edit inside it (everything else dimmed, a visible way out), a group moves and transforms as one with the box of 0019; no group style or transform. The grouping half of the old slice `layers-and-grouping`; layers are 0039. Needs a document-model decision by the customer. | Draft (criteria complete; ADR needs-customer, UX notes pending) | Must | R-EDIT-009 | 10 |
| [0030](0030-document-size-presets/) | `document-size-presets` | Paper sizes A0 to A6 and slide formats 16:9, 16:10, 4:3 as inline buttons in the Document section, a portrait/landscape switch, the selected format derived from the size; the list comes from one validated data file. Delivered inside the 0017 branch as its last milestone (one PR). | In progress (#78) | Should | R-EDIT-019 | - |
| [0031](0031-segment-drag-bending/) | `segment-drag-bending` | In the Node tool, drag a line or curve segment to bend it; the two handles move by a written rule, node types are honoured, blue/black preview, one commit, Escape cancels. | Ready | Should | R-EDIT-020, R-EDIT-001 | - |
| [0034](0034-pen-path-extension/) | `pen-path-extension` | With the Pen: continue an open path from either end, connect two paths by drawing onto the other's end, close with a sharp or smooth closing node (Shift flips, chip and preview show which), and a Close path command in the Node bar. Milestone M3 of the path-tools slice (one PR with 0016's toolbox fix, 0031 and 0035). | Ready | Should | R-EDIT-022 | - |
| [0035](0035-combine-and-break-apart/) | `combine-and-break-apart` | Combine closed shapes into one compound path (shapes inside shapes become holes, curves kept, crossing outlines refused); Break apart a compound path into pieces that keep their holes. Brings the second rail column (Path card). Milestone M4 of the path-tools slice (one PR with 0016's toolbox fix, 0031 and 0034). | Ready | Should | R-EDIT-023 | - |
| [0036](0036-split-at-crossings/) | `split-at-crossings` | Cut the selected paths into separate open pieces wherever they cross or touch each other or themselves, on the exact curves. The customer's word "Split" is interpreted; see its Question 1. | Draft (criteria complete; ADRs and UX notes pending) | Should | R-EDIT-023 | - |
| [0037](0037-fracture-and-flatten/) | `fracture-and-flatten` | Fracture: cut overlapping shapes into the pieces the overlaps make. Flatten: trim every shape to its visible part and remove hidden shapes. | Draft (criteria complete; ADRs and UX notes pending) | Should | R-EDIT-023 | - |
| [0038](0038-path-offset/) | `path-offset` | Outset and Inset by a typed distance in mm with Round, Miter or Bevel corners, open paths grow into a closed outline, live blue/black preview, the original kept. Needs a kernel decision (ADR 0003 §4). | Draft (criteria complete; ADRs and UX notes pending) | Should | R-EDIT-007 | - |
| [0040](0040-document-background/) | `document-background` | The document has a background fill: solid colour with alpha or none (drawn as a checkerboard), default today's #E8E8EB; a Background block in the Document section with the inline colour block of 0017; the eyedropper picks the background where no painted object is under the pointer, and the Background block has its own eyedropper. Needs a format version bump. Builds on 0017 and 0030 (same PR #78). | Ready (spec, `adrs.md` and UX notes exist; one PR, four milestones; #79 has merged, starts after #80 merges; claims `format_version` 10) | Should | R-EDIT-024 | - |
| [0041](0041-object-history/) | `object-history` | The history of one selected object (a scope in the History tab), object undo and redo with Ctrl+U and Ctrl+Shift+U (an object undo is a step that Ctrl+Z takes back, which makes it an object redo), a clone keeps a copy of its original's history, a wipe of one object's history, groups and multi-object steps. Customer text of 2026-10-10. Builds on `0020`. | Ready (2026-10-10: ADR 0014 accepted with all defaults; `adrs.md` and UX notes exist); builds once `0020` milestone 2 has merged | Should | R-HIST-002, R-HIST-004 | - |
| [0042](0042-history-branches/) | `history-branches` | Steps taken back and then not redone become a branch (no linear redo until the conflict step is taken back); lanes in the history list; a Preview / Live switch to look at or go to an earlier version of an object, with a Restore button; a clone of the state at any row, also of deleted objects; step ids everywhere. Preview by default, Live by a switch (customer accepted the default, 2026-10-10). Builds on `0020` and `0041`. | Ready (2026-10-10: ADR 0014 accepted with all defaults, Preview by default; `adrs.md` and UX notes exist); builds once `0020` and `0041` have merged | Should | R-HIST-003 | - |
| [0043](0043-properties-tabs/) | `properties-tabs` | Microtabs on the Properties panel (Document, Style, later History) with icons and tooltips, a fixed strip, auto-switch rules between Document and Style and a sticky manual choice, one Tab stop with roving focus, Shift+Ctrl+F and Shift+Ctrl+H. Lets the maker reach Document while an object is selected. Customer request of 2026-10-10. Built before `0020` milestone 3. | In progress (#84, one PR with 0045) | Should | R-EDIT-025 | - |
| [0044](0044-editing-quick-wins/) | `editing-quick-wins` | Ctrl+A selects all objects of the context (Select tool); arrow keys nudge the selection by 1 mm, Shift+Arrow by 10 mm, a held key is one step. Collected from reviews; a Proposal until the customer accepts. | In progress (#87; the customer can veto at the demo) | Should | R-EDIT-026 | - |
| [0045](0045-document-formats-library/) | `document-formats-library` | The format list of `0030` becomes user-extendable: formats and groups the maker adds, edits and deletes in an inline form, a star per format that puts it in the quick selection (the `0030` strips become the favourites), groups that can be switched off (Paper on, Slides off by default), the full list as an inline expandable list (no popup), import and export of a formats file, the maker's file laid over the built-in TOML. Customer request of 2026-10-10; where the user file lives needs the customer. | In progress for the rectangular part (#84, one PR with 0043; user-formats storage on default A, a per-user file in the data directory, the customer may veto) | Should | R-EDIT-027 | - |
| [0046](0046-document-shapes/) | `document-shapes` | Non-rectangular document areas: ellipse and circle, rounded rectangle, and an outline loaded from SVG or DXF (key rings, labels, hoops); the size is the bounding box, the area is a guide and not a mask; a format is a name, a size and a shape. A sketch with open questions. Needs a document-model decision, `0045`, `0040`, the SVG import of `0024`, and a DXF importer nobody has specified. | Draft (sketch with `adrs.md` sketch and UX sketch; the document-model ADR is `needs-customer` and not yet written) | Should | R-EDIT-028 | - |
| [0047](0047-transform-polish/) | `transform-polish` | Three customer change requests of 2026-10-10. (1) Skew, a stretch of a multi-selection and a stretch of a single polygon or star convert the primitives that cannot represent the result to paths, with no warning, hint or refusal (reverses `0008` P1, `0019` question 2 and the corner-only resize of a lone polygon or star, `0005` criterion 11). (2) Every slider (the Ratio sliders, the colour area, the hue slider): a click sets the value at the pointer, a double-click opens value entry, a drag is unchanged. (3) The typed move's Absolute mode measures from the centre of the true bounding box by default; the box is drawn pale red during a move drag and while the entry is open; in Absolute nine points of the box (the four corners, the four edge midpoints and the centre) are red marks that can be clicked to choose the reference and execute the move; plain Return uses the centre; a 3 x 3 Reference control in the chip gives the keyboard route. One PR, three milestones (conversion, sliders, reference box). | Ready (spec and `adrs.md` exist; the UX notes are the product owner's draft and the ux-engineer confirms them before the build; criteria marked Proposal are vetoable at the demo; customer changes of 2026-10-10 applied: nine reference points, a lone polygon or star converts too; runs after #80, before or after 0020 in any order, not beside it) | Should | R-EDIT-029, R-EDIT-012, R-EDIT-015 | - |

### Specified but not built

Written as criteria in a feature's spec, then taken out of that feature's
slice (lead decision 2026-10-09). No folder and no number yet; each becomes its
own entry when the customer schedules it.

| Part of | What it would deliver | Status | Priority |
|---|---|---|---|
| 0016 `boolean-operations` | Hover preview of the result: the outline of the result drawn over the canvas while the pointer rests on a Boolean button (criteria P1 to P3 in the 0016 spec). A Proposal, never accepted; no numbered criterion of 0016 depends on it. | Not started | Should |

### Draft (waiting for the customer to schedule)

| No. | Slug | What it delivers | Status | Priority | Requirements | MVP slice |
|---|---|---|---|---|---|---|
| [0021](0021-ellipse-arcs-and-shaping/) | `ellipse-arcs-and-shaping` | Ellipse arcs, and a Curve handle on ellipse, polygon and star. | Draft | Should | R-EDIT-002 | - |
| [0022](0022-color-management/) | `color-management` | Palettes, thread palettes for embroidery, colour history, other colour models. A sketch for the customer, after the MVP. | Draft | Could | R-EDIT-017 | - |
| [0032](0032-pen-tablet-input/) | `pen-tablet-input` | Pen pressure (and tilt, eraser end) from a drawing tablet changes the width of freehand strokes; needs a freehand tool first, a per-platform feasibility check (Linux webview) and a document-model decision. "Later, not now" (customer). | Draft | Could | R-INP-001 | - |
| [0033](0033-stroke-brushes/) | `stroke-brushes` | Brushes for the stroke (width profile, stamp, pattern along path), a picker in the Stroke section, brushes as data files the maker can add, plugin brushes later; expanded to paths for jobs. After the MVP, 0017, 0018 and the plugin host. | Draft | Could | R-EDIT-021 | - |

### MVP slices not yet specified (no folder, number reserved)

| No. | Slug | What it delivers | Status | Priority | Requirements | MVP slice |
|---|---|---|---|---|---|---|
| 0024 | `svg-import-export` | Open a plain SVG from Inkscape and export it again without hand-fixing geometry; a named, listed loss report for anything outside our supported subset. | Not started | Must | R-SYS-006 | 11 |
| 0025 | `raster-trace` | Trace a raster image to vector paths with adjustable threshold and colour count, on par with Inkscape's "Trace Bitmap". | Not started | Must | R-VEC-001 | 12 |
| 0026 | `machine-profile` | Define and reuse a machine profile (work area, connection, limits) for a laser cutter; select it for a project. | Not started | Must | R-MFG-001 | 13 |
| 0027 | `manufacturing-roles` | Assign a cut or engrave role to geometry (by layer) within one file. | Not started | Must | R-MFG-002 | 14 |
| 0028 | `material-test-library` | Generate a power/speed test-cut grid for the selected machine, record which cell worked, store it as a reusable material record. | Not started | Must | R-MAT-001, R-MAT-002, R-MAT-003 | 15 |
| 0029 | `laser-job-preview-and-output` | Toolpath and time preview, an explicit machine and material gate before export, GRBL G-code export for cut and engrave geometry. The MVP's capstone: a maker's own design goes from drawing to a file their laser runs. | Not started | Must | R-MFG-003, R-MFG-LASER-001, R-SYS-008 | 16 |
| 0039 | `layers` | Layers with per-layer visibility and lock, to separate cut, engrave and reference geometry; the second half of the old slice `layers-and-grouping` (the first half is `0023-groups`). Number assigned 2026-10-10 when 0023 was given to groups. | Not started | Must | R-EDIT-009 | 10 |

Not in this list: Should and Could requirements, and every Must the customer
deferred past the laser MVP on 2026-10-02 (multi-OS parity and sync, fonts,
asset management, collaboration, the asset connector, plugins, other machine
families). They are tracked in `docs/requirements.md` ("MVP (confirmed)" and
"Explicitly deferred past MVP") and get a number here once the customer
prioritises them. The next free number is 0048.

## Proposed build order (product owner, 2026-10-10)

For 0023 and 0030 to 0038, with the fixed points 0016 (done), 0017, 0018, 0019 and the reserved 0020. The lead decides; this is my reading of dependencies and of which slices can run side by side (at most two implementers, `CLAUDE.md` §4). Almost every slice touches `curvyo-editor-wasm` and `frontend`, so "same crate" cannot be the test; the table names the files and modules that actually collide.

| Spec | Main footprint | Needs first | Collides with |
|---|---|---|---|
| 0030 `document-size-presets` | `document-core` (new module and data file), `editor-wasm` `session/document.rs`, the Document section of `PropertiesPanel.tsx` | 0015 (merged) | 0017 (same panel file): after 0017 |
| 0031 `segment-drag-bending` | `ui-core` `node_tool.rs`, `render-core` preview, `editor-wasm` `session/node.rs`, `document-core` (one commit) | none | 0034 (both amend `0002`; separate modules) |
| 0034 `pen-path-extension` | `ui-core` `pen_tool.rs` and hit test, `render-core` pen preview, `editor-wasm` `session/pen.rs` and `draw.rs`, hint chip, Node and Select bars | none | 0031 (see above); the bars with 0035 to 0038 |
| 0023 `groups` | `document-core` (tree, format), `storage-io`, `ui-core` selection, hit test and boxes, `render-core` dimming, `editor-wasm`, `frontend` | 0019 (group box), 0016; customer's model decision | 0019, and every spec that selects objects |
| 0035 `combine-and-break-apart` | `document-core` compound ops, `geometry-core` nesting and crossing test, rail buttons | 0016 merged; the rail layout decision | 0036, 0037, 0038 only through the rail |
| 0036 `split-at-crossings` | `geometry-core` (new curve-intersection module), rail button | 0016 merged; the rail layout decision | 0037, 0038 (same crate) |
| 0037 `fracture-and-flatten` | `geometry-core` (kernel use), rail buttons | 0016 merged; the rail layout decision | 0036, 0038 (same crate) |
| 0038 `path-offset` | `geometry-core` (offset, maybe a new dependency), an entry widget, `render-core` preview | 0016 merged; the kernel check of ADR 0003 §4 | 0036, 0037 (same crate) |
| 0040 `document-background` | `document-core` (two registers, format), `ui-core` Document view and eyedropper hit test, `render-core` document rectangle and checkerboard, `editor-wasm`, the Document section of `PropertiesPanel.tsx` | 0017 and 0030 merged (PR #78): it reuses their colour block, eyedropper and Document section | 0023 (format version, document root); 0031 and 0034 are disjoint |
| 0043 `properties-tabs` | `frontend` `PropertiesPanel.tsx` (strip, tabpanel, auto-switch), the panel-context view in `editor-wasm` `wasm_document.rs`, design-system rows | none (`adrs.md`: it changes the panel frame and the body selector only, so it can start from `main`; rebase-level conflict with #80) | the Document and Style sections of 0017, 0030, 0040, 0045 (same panel file): order 0043, then 0040, then 0045 |
| 0020 `undo-redo` | `document-core` (step data, names, restore engine, stack machine, wipe), `editor-wasm` (every commit path, key gate in `session/keys.rs`, selection restore), `frontend` (History tab, notices, Edit menu in `curvyo-app`), removal of "No undo yet" in six spec files and five frontend text files | ADR 0014 accepted (2026-10-10); 0043 (merged, #84) for milestone 3. The spike (milestone 0) is done | every spec that adds a commit path: it runs alone (wave 4). Milestones 1 and 2 do not need 0043 |
| 0041 `object-history` | `document-core` (per-object index, `history_floor` and `clone_of` keys, object undo), `editor-wasm`, History tab scope | 0020 milestone 2 merged (ADR 0014 accepted 2026-10-10; milestones 3 and 4 write the optional keys) | 0023 (object identity, groups), 0042 (same list component and model) |
| 0042 `history-branches` | `document-core` (branch derivation, state at a version, preview fork, clone), `render-core` preview, `editor-wasm`, History tab lanes | 0020, 0041 merged; ADR 0014 accepted | 0041; the preview touches the renderer, which 0023 also touches (dimming) |
| 0044 `editing-quick-wins` | `editor-wasm` `session/keys.rs`, `ui-core` selection, shortcut table, a per-version object cache | #80 merged (group box, multi-object move, key table). Without 0020 a held nudge writes one commit per event; with 0020 they form one step | 0020 and 0043 (the same key gate and panel files): not in parallel; any other slice is free |
| 0045 `document-formats-library` | `document-core` (loader, schema 2, overlay), `curvyo-storage-io` (user file, atomic write), `curvyo-app` commands, `editor-wasm` view, the Document tab | 0030 merged; 0040 and 0043 merged for milestones 2 and 3 (milestone 1, the loader, may start earlier); the user-file place on default A unless the customer vetoes | 0040 and 0043 (the Document section file) |
| 0046 `document-shapes` | `document-core` (shape register, format bump), `render-core` (area, edge), importers in `0024` (SVG) and an unspecified DXF reader, the Document tab | 0045, 0040, the customer and architect on the model; stage 2 needs 0024, stage 3 a DXF importer | 0023 (format version, document root), 0040 (background fill of the area) |
| 0047 `transform-polish` | `ui-core` (skew and stretch conversion rule, `move_entry.rs` reference, new `reference_point.rs`), `document-core` (`transform_objects` with a conversion), `render-core` (reference box and marks), `editor-wasm` (move drag decoration, chip), `frontend` (four sliders, `MoveEntryChip.tsx`) | #80 merged (group box); #78 merged (picker). Not waiting for 0020: the two run in either order (customer, 2026-10-10) | `0044` and `0020` (key gate, commit labels, `MoveEntryChip.tsx`), `0043` and `0040` (`useEditorSession.ts`, panel files): not in parallel with them |
| 0032 `pen-tablet-input` | pointer input in `frontend` and `editor-wasm`, possibly the Tauri host; the stroke width model | a freehand tool (no spec yet), the platform spike | 0033 (width model) |
| 0033 `stroke-brushes` | style model, `render-core`, `geometry-core` generators, panel row, plugin host | 0017, 0018, the plugin host | 0018, 0032 (style and width model) |

**Waves, two slices at a time after 0017** (a slice from a wave may start as soon as its own "needs first" is merged):

1. 0017 (0016 is done; its toolbox amendment is a small `fix/` PR).
2. 0019 and 0030. (0030 after 0017 merges.)
3. 0031 and 0018. Disjoint: node tool against style model and panel.
4. Undo and history, in this order: **0043 → 0020 → 0041 → 0042.** 0043 `properties-tabs` first (small, can start from `main`; 0020's History tab needs it). Then 0020 `undo-redo`, alone: it touches every commit path, and every new command above would otherwise ship with "No undo yet." Its spike is done; milestone 3 needs 0043 (merged, #84); ADR 0014 is accepted (2026-10-10), so nothing waits for the customer. 0047 `transform-polish` runs before or after 0020, in either order, but not beside it (one key gate, the commit labels, the move chip). Then 0041 (after 0020 milestone 2), then 0042. 0044 `editing-quick-wins` builds after #80 merges, before or after 0020 but not beside it or beside 0043 (one key gate, one panel). 0034 to 0038 ship with the notices as written if they merge before 0020; 0020's criterion 52 removes them.
4a. 0045 `document-formats-library` follows 0040 and 0043 (the same Document tab file); its loader milestone (`document-core` only) may start earlier. The user file's place is on default A unless the customer vetoes. It can run next to 0020 only if 0020's footprint (`document-core` step data, `keys.rs`) is disjoint from the loader and `curvyo-storage-io`, which it is. 0046 `document-shapes` comes last: it follows 0045 and 0040; stage 1 (circle, rounded rectangle) needs the model decision, stage 2 needs 0024, stage 3 a DXF importer that is not specified (customer question).
5. 0034 and 0035. Disjoint: Pen against compound outlines. 0035 needs the rail layout decision (below).
6. 0023 and 0038. Disjoint: document model and selection against the kernel. 0038 needs the ADR 0003 §4 check first, 0023 the customer's model decision.
7. 0036, then 0037, one after the other (both extend `geometry-core`), each next to a slice that does not (0039 `layers` after 0023, or an MVP slice such as 0024).
8. After the MVP: the freehand tool and 0032 (after a spike with a real tablet), then 0033 (after the plugin host).

**One decision that blocks the UI of five slices: the left rail is full.** With the Boolean section it is 501 px high and the 800 x 600 viewport has 546 px (`0016` Question 1). Group and Ungroup (0023), Combine and Break apart (0035), Split (0036), Fracture and Flatten (0037) are nine more command buttons, and Offset (0038) a tenth. The ux-engineer decides once, before 0035 starts: two columns, a scrolling rail, collapsible command sections, or moving some commands to the Select bar. The specs say "where they sit is the ux-engineer's decision" and define behaviour only.

## The MVP and its sequence

The laser MVP (`docs/requirements.md`, "MVP (confirmed)") is the 16 slices
that carry an entry in the "MVP slice" column. Scope: **laser-only,
local-only, single-user, one controller dialect** (GRBL-family G-code),
confirmed by the customer on 2026-10-02. Cutting plotter, embroidery, CNC,
real-time collaboration, the asset connector and the plugin interface are each
later slices once the MVP ships.

Every MVP slice is **Must** priority; the MVP has no Should or Could. A slice
may fold in a `Proposal` requirement that only makes sense next to the
Customer one beside it.

Two requirements are not their own slice because every slice must already
satisfy them: **R-SYS-002** (offline-capable) and **R-SYS-007** (no
telemetry). A slice that needs the network, or phones home for anything the
maker did not trigger, has a bug, not a variant.

### Spec number versus slice label

The old MVP list numbered the slices 1 to 16 and gave folders the same
number. Since the customer asked for one consecutive numbering of all specs,
the folder number now follows the list above, and the slice label lives in the
"MVP slice" column. The two agree for 0001 to 0007. After that they differ:
slice 8 `undo-redo` is 0020, slice 9 `boolean-operations` is 0016, slice 10
`layers-and-grouping` is split into 0023 `groups` and 0039 `layers`, and slices
11 to 16 are 0024 to 0029. Existing text that says "slice 5" or "slice 8"
means the old label. Text that says `0005` or `specs/0005-object-transform/`
means the folder.

### Order and dependencies between MVP slices

Slice labels, as in the column above.

- **1 before everything**: nothing to draw on or persist without a project
  file and a canvas. It uses the `.curvyo` container and `document.loro`
  backing decided in ADR 0004 §1.
- **2 before 3**: primitives are "editable as paths after creation"
  (R-EDIT-002) and "object to path" (R-EDIT-004) needs a path-editing surface
  to convert into. The shape tools are a thin layer over it.
- **4 to 6 were inserted after the customer tried slices 1 to 3 by hand**
  (2026-10-05): pan/zoom and a general Select tool (4), scale and rotate by
  handles (5), join, split and a third node type (6). They run before styling
  at the customer's request, because styling is hard to judge on objects you
  cannot select, transform or view at a useful zoom.
- **8 (`undo-redo`) after 2 to 3 and 5 to 7, before 9 and 10**: undo is
  cross-cutting (ADR 0002 §9, ADR 0004 §2, peer-scoped and CRDT-backed), but
  "covers every editing operation" (R-EDIT-008) can only be checked once
  several operations exist. Pan, zoom and selection are view state and have no
  undo surface. Slices 9 and 10 each add a criterion that their new operation
  is undoable through the mechanism slice 8 sets up, rather than repeating its
  work. The list above does not follow this order: 0016 (slice 9) was specified
  ahead of it. The customer's undo text arrived on 2026-10-10 and is specified in
  0020 (slice 8), with the object history and branches in 0041 and 0042. How
  0016 handled its undo criterion until 0020 exists was 0016's own decision.
- **11 (SVG round trip) after 7 and 10**: round-tripping a real Inkscape file
  is only a meaningful test once paths, primitives, styling, groups and
  layers exist.
- **13 to 16 last and in that order**: a job needs a machine profile (13)
  before roles can target one (14); roles and a machine before a material test
  is worth running (15); all three before the capstone export (16), where
  R-SYS-008's "no job without a machine profile" gate applies.

### `format_version` plan

The project file's `format_version` is **9** on `main` (set by 0017 and 0018,
PR #78). PR #90 (0040) takes **10**.
Each change that alters the on-disk shape takes the next number at merge, and
a reader refuses a file with a higher number ("saved by a newer version").

| Version | Taken by | State |
|---|---|---|
| 7 | 0007 `stroke-and-fill-styling` | Done, merged (#54) |
| 8 | 0016 `boolean-operations` (compound path, PR #73) | Done, merged |
| 9 | 0017 `style-panel-rework` (odd dash lists) and 0018 `stroke-markers`, one PR | On `main` (#78) |
| none | 0015 `document-size-and-rulers` | No bump (its `adrs.md`, decision 1) |
| 10 | 0040 `document-background` (background paint and colour registers in the document root) | Built in PR #90 (open); claims 10, the next free number: the open PR #87 keeps 9. If another bump merges first, 0040 takes the next free one (not built in parallel with 0023) |
| next free at merge | 0023 `groups` (a group node with children) | Planned; 11 if 0040 merges first; a document-model change that needs the customer |
| next free at merge | 0033 `stroke-brushes` (brush reference and embedded definition) | Draft, after the MVP |
| none (ADR 0014 Q2 A, accepted 2026-10-10) | 0041 `object-history` (the optional keys `history_floor` and `clone_of`, ADR 0014 §7) | Ready; the keys are additive and older builds ignore them (as 0015's `display_unit`); the file keeps the version it has when 0041 merges (10 once #90 has merged) |
| next free at merge | 0046 `document-shapes` (the `page_shape` register; an older build would draw a rectangle where a key ring is) | Draft; `needs-customer` |
| none | 0020, 0042, 0043, 0044, 0045, 0047 | 0020 saves the history that `document.loro` already holds (step headers in commit messages) and adds one optional root key, `history_wiped` (ADR 0014 §7); 0042 derives branches from the log; 0043 and 0044 write nothing; 0045 keeps formats in a user file outside the project; 0047 writes paths, an existing kind. 0020, 0041 and 0042 keep whatever version is current when they merge: 10 once #90 has merged. A bump for history would come only with soft delete (ADR 0014 Q7 B), which the customer gets back only if a Loro upgrade breaks the canary |
| none | 0030, 0031, 0034, 0035, 0036, 0037, 0038 | No bump: they write existing object kinds (0035 to 0037 use the compound path of 0016) |

A spec does not hard-code a version number it does not own; it says "next free
at merge". A slice names a number only when it is the next bump to merge: 0016
named 8, 0040 names 10. If another bump merges first, that slice's tests
compare against its own named constant and the rebase renumbers it; this table
is then corrected in the same PR.

## Numbering

`NNNN` is a 4-digit, zero-padded number, the same style `docs/adr/NNNN-slug.md`
uses for ADRs. The number is the folder's position in the feature list above,
assigned when the folder is created: take the next free number (or the reserved
number of a placeholder row) and add the row to the list in the same PR.

On 2026-10-09 the customer had every folder renumbered into one consecutive
sequence (built features in merge order, then the planned ones). Before that,
only the MVP slices had numbers. That renumbering was a one-time decision by
the customer; no one renumbers on their own. Folders keep their number after
it is assigned, even if the list is later regrouped.

Branches, PR titles and prose refer to a feature by its slug alone
(`story/<slug>`), with no number. Paths and links use the full
`NNNN-slug`.

## Keeping the list current

The product owner owns this list. Change it in the same PR that changes the
fact:

- a new folder adds its row (status, priority, requirements);
- a status change in `specification.md` changes the row in the same PR;
- when a story is accepted: `Status: Done` in the spec, the PR numbers in the
  row and in the spec's Links.

A status in a row that disagrees with the spec's `Status:` line is a bug in
the row.

## Layout of a feature folder

One folder per feature: `specs/<NNNN-feature-slug>/`, three files.

### `specification.md` — what and why (product-owner)

```text
# <feature title>

Status: Draft | Ready | In progress | Done
Priority: Must | Should | Could
Origin: Customer | Proposal

## User value
As a <maker role> I want <capability> so that <outcome>.

## Acceptance criteria
1. Given ..., when ..., then ...

## Out of scope
- ...

## UX notes
(filled in by ux-engineer before Ready)

## Links
Requirements: ...  PR: ...
```

### `adrs.md` — which decisions apply (architect)

```text
# ADRs for <feature title>

- [ADR 000X](../../docs/adr/000X-slug.md): <one line on what it decides
  for this feature>

## Feature-local decisions
Anything too small for a full ADR, dated:

- YYYY-MM-DD: <decision> — <why>
```

### `plan.md` — how it gets built (implementer)

```text
# Plan for <feature title>

## Affected crates/modules
- ...

## Tasks
- [ ] 1. <task> (fulfils AC 1, 2)
- [ ] 2. <task> (fulfils AC 3)

## Validation
How this gets verified beyond the tester's acceptance tests (e.g. golden
files, property tests, manual check with hardware).
```

## Lifecycle

Same stages as `CLAUDE.md` §4:

1. **Ready** — `specification.md` + `adrs.md` exist, UX notes attached if
   the feature has UI.
2. **Build** — implementer writes `plan.md`, then works on branch
   `story/<feature-slug>` in a worktree, checking off tasks as it goes.
3. **Verify** — tester works from `specification.md` only; `ux-engineer`
   and `architect` review per `CLAUDE.md` §4.
4. **Demo** / **Done** — as in `CLAUDE.md` §4; `specification.md` gets
   `Status: Done` and the PR link.

**One PR per slice (customer rule, 2026-10-09).** A slice is one story and
one PR on one branch. A large slice is built as milestones (commits on that
branch, each green), not as several PRs; the customer accepts the slice once,
against its review guide. A part the customer does not need for the slice moves
out as its own entry (see "Specified but not built").

A feature folder's `specification.md` is the single source of truth for
what "done" means. `plan.md` tracks progress; it is not a second backlog.
