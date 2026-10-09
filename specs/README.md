# Feature specs

The list of all features comes first, then the convention for writing them.
State of this list: 2026-10-10 (the customer renumbered every folder on
2026-10-09, see "Numbering" below; the customer requests of 2026-10-09 added
0030 to 0039 and gave the groups half of 0023 its spec).

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
`Draft` until `adrs.md` and the UX notes come. Their
order, dependencies and conflicts are in "Proposed build order" below.

| No. | Slug | What it delivers | Status | Priority | Requirements | MVP slice |
|---|---|---|---|---|---|---|
| [0015](0015-document-size-and-rulers/) | `document-size-and-rulers` | Rulers in mm along the top and left, document resize with the content staying centred, fit the document to the drawing, drawing outside the document edge. | Done (#67, #72) | Must | none yet | - |
| [0016](0016-boolean-operations/) | `boolean-operations` | Union, difference, intersection, exclusion and reverse difference on closed paths, as a command section in the left tool rail; compound-path results. One PR (#73), built as milestones on one branch. The hover preview of the result is not part of it (see "Specified but not built" below). Amended 2026-10-10: the Boolean operations become their own toolbox card (small `fix/` PR). | Done (#69, #73) | Must | R-EDIT-003 | 9 |
| [0017](0017-style-panel-rework/) | `style-panel-rework` | Empty panel when nothing is selected, controls hidden instead of disabled, 8-digit RGBA hex, inline colour picker, eyedropper, custom dash text line, GIMP-style value fields, gradients removed. Replaces parts of 0007. | Ready | Must | R-EDIT-005, R-EDIT-006 | - |
| [0018](0018-stroke-markers/) | `stroke-markers` | Arrow or dot at the start, the end, N places along and on every node of a path. Builds on 0017. | Ready | Should | R-EDIT-005, R-EDIT-016 | - |
| [0019](0019-multi-object-transform/) | `multi-object-transform` | One group box with the same handles as a single object, for a selection of several objects. Runs after 0017; 0014 is merged. | Ready | Should | R-EDIT-012 | - |
| 0020 | `undo-redo` | Ctrl+Z and Ctrl+Y undo and redo every editing operation, one interaction = one undo step. **Number reserved, no folder: the customer has ideas that come first.** | Not started | Must | R-EDIT-008 | 8 |
| [0023](0023-groups/) | `groups` | Group and ungroup, nested groups, enter a group to edit inside it (everything else dimmed, a visible way out), a group moves and transforms as one with the box of 0019; no group style or transform. The grouping half of the old slice `layers-and-grouping`; layers are 0039. Needs a document-model decision by the customer. | Draft (criteria complete; ADR needs-customer, UX notes pending) | Must | R-EDIT-009 | 10 |
| [0030](0030-document-size-presets/) | `document-size-presets` | Paper sizes A0 to A6 and slide formats 16:9, 16:10, 4:3 as inline buttons in the Document section, a portrait/landscape switch, the selected format derived from the size; the list comes from one validated data file. Delivered inside the 0017 branch as its last milestone (one PR). | Ready | Should | R-EDIT-019 | - |
| [0031](0031-segment-drag-bending/) | `segment-drag-bending` | In the Node tool, drag a line or curve segment to bend it; the two handles move by a written rule, node types are honoured, blue/black preview, one commit, Escape cancels. | Ready | Should | R-EDIT-020, R-EDIT-001 | - |
| [0034](0034-pen-path-extension/) | `pen-path-extension` | With the Pen: continue an open path from either end, connect two paths by drawing onto the other's end, close with a sharp or smooth closing node (Shift flips, chip and preview show which), and a Close path command in the Node bar. Milestone M3 of the path-tools slice (one PR with 0016's toolbox fix, 0031 and 0035). | Ready | Should | R-EDIT-022 | - |
| [0035](0035-combine-and-break-apart/) | `combine-and-break-apart` | Combine closed shapes into one compound path (shapes inside shapes become holes, curves kept, crossing outlines refused); Break apart a compound path into pieces that keep their holes. Brings the second rail column (Path card). Milestone M4 of the path-tools slice (one PR with 0016's toolbox fix, 0031 and 0034). | Ready | Should | R-EDIT-023 | - |
| [0036](0036-split-at-crossings/) | `split-at-crossings` | Cut the selected paths into separate open pieces wherever they cross or touch each other or themselves, on the exact curves. The customer's word "Split" is interpreted; see its Question 1. | Draft (criteria complete; ADRs and UX notes pending) | Should | R-EDIT-023 | - |
| [0037](0037-fracture-and-flatten/) | `fracture-and-flatten` | Fracture: cut overlapping shapes into the pieces the overlaps make. Flatten: trim every shape to its visible part and remove hidden shapes. | Draft (criteria complete; ADRs and UX notes pending) | Should | R-EDIT-023 | - |
| [0038](0038-path-offset/) | `path-offset` | Outset and Inset by a typed distance in mm with Round, Miter or Bevel corners, open paths grow into a closed outline, live blue/black preview, the original kept. Needs a kernel decision (ADR 0003 §4). | Draft (criteria complete; ADRs and UX notes pending) | Should | R-EDIT-007 | - |

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
prioritises them. The next free number is 0040.

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
| 0032 `pen-tablet-input` | pointer input in `frontend` and `editor-wasm`, possibly the Tauri host; the stroke width model | a freehand tool (no spec yet), the platform spike | 0033 (width model) |
| 0033 `stroke-brushes` | style model, `render-core`, `geometry-core` generators, panel row, plugin host | 0017, 0018, the plugin host | 0018, 0032 (style and width model) |

**Waves, two slices at a time after 0017** (a slice from a wave may start as soon as its own "needs first" is merged):

1. 0017 (0016 is done; its toolbox amendment is a small `fix/` PR).
2. 0019 and 0030. (0030 after 0017 merges.)
3. 0031 and 0018. Disjoint: node tool against style model and panel.
4. 0020 `undo-redo`, alone: it touches every commit path, and every new command above would otherwise ship with "No undo yet." If the customer's undo ideas are not ready, skip to 5 and let 0034 to 0038 ship with the notices as written.
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
  work. The list above does not follow this order: the customer has ideas for
  undo that come first (0020 is reserved), and 0016 (slice 9) is specified
  ahead of it. How 0016 handles its undo criterion until 0020 exists is
  0016's own decision.
- **11 (SVG round trip) after 7 and 10**: round-tripping a real Inkscape file
  is only a meaningful test once paths, primitives, styling, groups and
  layers exist.
- **13 to 16 last and in that order**: a job needs a machine profile (13)
  before roles can target one (14); roles and a machine before a material test
  is worth running (15); all three before the capstone export (16), where
  R-SYS-008's "no job without a machine profile" gate applies.

### `format_version` plan

The project file's `format_version` is **7** on `main` (set by 0007, PR #54).
Each change that alters the on-disk shape takes the next number at merge, and
a reader refuses a file with a higher number ("saved by a newer version").

| Version | Taken by | State |
|---|---|---|
| 7 | 0007 `stroke-and-fill-styling` | On `main` |
| 8 | 0016 `boolean-operations` (compound path, PR #73) | Done, merged |
| 9 | 0017 `style-panel-rework` (odd dash lists) and 0018 `stroke-markers`, one PR | In progress (`story/style-panel-rework`) |
| none | 0015 `document-size-and-rulers` | No bump (its `adrs.md`, decision 1) |
| next free at merge | 0023 `groups` (a group node with children) | Planned; a document-model change that needs the customer |
| next free at merge | 0033 `stroke-brushes` (brush reference and embedded definition) | Draft, after the MVP |
| none | 0030, 0031, 0034, 0035, 0036, 0037, 0038 | No bump: they write existing object kinds (0035 to 0037 use the compound path of 0016) |

A spec does not hard-code a version number it does not own; it says "next free
at merge". Only 0016 names a number (8), because it is the one that merges
first. If a third feature merges before 0016, 0016's tests compare against its
own named constant and the rebase renumbers it; this table is then corrected
in the same PR.

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
