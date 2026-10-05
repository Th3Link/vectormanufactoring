# MVP feature sequence

The laser MVP (`docs/requirements.md`, "MVP (confirmed)") broken into thin,
independently-triable vertical slices, in dependency order. This is the
single place to see the MVP as a sequence of features — do not re-derive it
from `docs/requirements.md` each round; update this file instead when the
sequence changes.

Scope: **laser-only, local-only, single-user, one controller dialect**
(GRBL-family G-code), as confirmed by the customer on 2026-10-02. Cutting
plotter, embroidery, CNC, real-time collaboration, the asset connector and
the plugin interface are each later slices once this sequence ships — see
`docs/requirements.md`'s "Explicitly deferred past MVP" and
"MVP-sequencing note".

Every slice below is **Must**-priority (the MVP has no Should/Could — those
are later requirements, not later MVP slices). All `Origin: Customer`
requirements map directly; a few slices additionally fold in a `Proposal`
requirement (noted) that only makes sense bundled with the Customer one next
to it.

Two requirements are **not** their own slice because they are cross-cutting
constraints every slice must already satisfy, not a feature with its own
UI to try: **R-SYS-002** (offline-capable) and **R-SYS-007** (no telemetry).
Each slice's acceptance criteria assume both; a slice that needs network
access or phones home for anything beyond what the maker explicitly
triggered is a bug, not a variant.

| # | Feature slug | Delivers | Requirements | Priority |
|---|---|---|---|---|
| 1 | `project-file-foundation` | Create, open and save a local `.vmf` project with an empty canvas; survives close/reopen, same result on relaunch. | R-SYS-001, R-SYS-002 | Must |
| 2 | `path-node-editing` | Draw and edit a Bézier path with a pen tool: add/move/delete nodes, drag handles, same node/handle/segment mental model as Inkscape. | R-EDIT-001 | Must |
| 3 | `primitive-shapes` | Rectangle (with corner radius), circle/ellipse and polygon/star tools; "object to path" converts any of them to an editable path. | R-EDIT-002, R-EDIT-004 | Must |
| 4 | `canvas-navigation-and-selection` | Pan/zoom the canvas (zoom toward cursor), stable behaviour across window resize, and a general Select tool to click/move/delete any object without re-entering its creation tool. | R-EDIT-010, R-EDIT-011 | Must |
| 5 | `object-transform` | Move, scale and rotate any path or primitive via on-canvas handles on the Select tool's bounding box, with proportional stroke-width/corner-radius scaling. | R-EDIT-012 | Must |
| 6 | `path-merge-split-and-node-types` | Join two path endpoints into one node, split a path at a node into two; a third node type (Asymmetric) alongside Corner and Symmetric. | R-EDIT-014 | Must |
| 7 | `stroke-and-fill-styling` | Stroke width/dash/join/cap/color; solid fill and linear/radial gradient fill, on any path or primitive. | R-EDIT-005, R-EDIT-006 | Must |
| 8 | `undo-redo` | Ctrl+Z/Ctrl+Y undoes/redoes every editing operation shipped in slices 2–3 and 5–7, one interaction = one undo step, no silent data loss. | R-EDIT-008 | Must |
| 9 | `boolean-operations` | Union, difference, intersection on closed paths. | R-EDIT-003 | Must |
| 10 | `layers-and-grouping` | Group/ungroup objects; layers with per-layer visibility and lock, to separate cut/engrave/reference geometry. | R-EDIT-009 | Must |
| 11 | `svg-import-export` | Open a plain SVG from Inkscape and re-export it without hand-fixing geometry; named, listed loss report for anything outside our supported subset. | R-SYS-006 | Must |
| 12 | `raster-trace` | Trace a raster image to vector paths with adjustable threshold/color-count, parity with Inkscape's "Trace Bitmap". | R-VEC-001 | Must |
| 13 | `machine-profile` | Define and reuse a machine profile (work area, connection, limits) for a laser cutter; select it for a project. | R-MFG-001 | Must |
| 14 | `manufacturing-roles` | Assign cut/engrave role to geometry (by layer) within one file. | R-MFG-002 | Must |
| 15 | `material-test-library` | Generate a power/speed test-cut grid for the selected machine, record which cell worked, store it as a reusable material record. | R-MAT-001, R-MAT-002, R-MAT-003 | Must |
| 16 | `laser-job-preview-and-output` | Toolpath/time preview, an explicit machine+material gate before export, and GRBL G-code export for cut + engrave geometry. The MVP's capstone: a maker's own design goes from drawing to a file their laser runs. | R-MFG-003, R-MFG-LASER-001, R-SYS-008 | Must |

## Notes on ordering

- **1 before everything**: there is nowhere to draw and nothing to persist
  without a project file and a canvas. Uses the `.vmf` container and
  `document.loro` backing already decided in ADR 0004 §1 — this slice does
  not reopen that decision, it is the first thing to exercise it.
- **2 before 3**: primitives are specified as "editable as paths after
  creation" (R-EDIT-002) and "object to path" (R-EDIT-004) presupposes a
  path-editing surface exists to convert *into*. Building the node/handle
  surface first also means the shape tools in 3 are a thin layer over
  already-working machinery, not a parallel implementation.
- **4 (canvas navigation and selection) inserted after 3, before the rest**:
  added 2026-10-05, after the customer tested slices 1–3 by hand and found
  two real gaps — the canvas has no pan/zoom, and there is no general
  Select tool, so an object made with a creation tool (pen, rectangle,
  ellipse, polygon/star) cannot be clicked, moved or deleted without
  re-entering that same tool. Sequenced before styling at the customer's
  explicit request since styling controls are hard to evaluate on objects
  you can't select or view at a useful zoom level. Not a hard dependency
  for 2–3 (both shipped and work without it), but every slice from 5
  onward benefits from a working Select tool to test against.
- **5 (object transform) inserted immediately after 4**: added the same
  day, same customer round — plain move (slice 4's AC20) isn't enough, the
  customer wants scale and rotate too, via on-canvas handles on the Select
  tool's own bounding box. Directly builds on 4's `ObjectSelection`/
  `ViewTransform`; sequenced before styling for the same "can't evaluate
  what you can't select and transform" reason as slice 4. Adds a
  `rotation` register to the document model (ADR 0002 §5's deferred
  per-node transform question, finally due) — `format_version` goes to 4
  as a direct result; `stroke-and-fill-styling` moves to `format_version`
  5 behind it, a dated note in its own `adrs.md`.
- **6 (path merge/split and node types) inserted immediately after 5**:
  added the same day as a third customer ask in the same round — Join,
  Split and a third node type (Asymmetric, alongside Corner and a
  relabelled Symmetric — Symmetric is slice 2's existing `Smooth`, renamed
  for clarity now that a third kind exists). Builds on slice 4's Select
  tool for cross-object Join (select two path objects, then switch to Node
  tool to pick the specific endpoints) and on slice 5's `rotation` register
  (a joined/split object's rotation must carry over correctly). Sequenced
  before styling for the same reason as 4 and 5. `format_version` goes to
  5; `stroke-and-fill-styling` moves to `format_version` 6 behind it, a
  dated note in its own `adrs.md`. The `AnchorKind` enum gains a variant
  (`Corner`/`Symmetric`/`Asymmetric`) — `Smooth`'s on-disk tag is renamed to
  `symmetric` going forward, with a documented one-line migration (an old
  file's `smooth` tag still reads as `Symmetric`, never rewritten in place).
- **8 (undo) after 2–3 and 5–7, before 9–10**: undo is cross-cutting
  infrastructure (ADR 0002 §9, ADR 0004 §2 — peer-scoped, CRDT-backed) but
  "covers every editing operation" (R-EDIT-008) is only checkable once
  there is more than one operation to undo. Slice 4's pan/zoom/selection is
  view state, not document state, so it has no undo surface and is not
  part of this dependency; slices 9 and 10 each add an acceptance criterion
  that their new operation is undoable through the mechanism slice 8
  establishes, rather than repeating slice 8's work.
- **11 (SVG round-trip) after 7 and 10**: round-tripping a real Inkscape
  file is only a meaningful test once paths, primitives, styling, groups
  and layers all exist to round-trip. Earlier would mean testing against
  near-empty documents.
- **13–16 last and in that order**: a job needs a machine profile (13)
  before roles can target one (14), roles and a machine before a material
  test is worth running (15), and all three before the capstone job-preview
  export story (16) — which is where R-SYS-008's "no job without a machine
  profile" gate actually bites.

## Out of sequence (tracked in `docs/requirements.md`, not listed above)

Should/Could requirements, and every Must deferred past the laser MVP by
the customer's 2026-10-02 confirmation (multi-OS parity/sync, fonts, asset
management, collaboration, the asset connector, plugins, other machine
families) — see "MVP (confirmed)" and "Explicitly deferred past MVP" in
`docs/requirements.md`. These get their own slices, sequenced after #13, once
the customer prioritizes them.
