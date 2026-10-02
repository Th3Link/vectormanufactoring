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
| 4 | `stroke-and-fill-styling` | Stroke width/dash/join/cap/color; solid fill and linear/radial gradient fill, on any path or primitive. | R-EDIT-005, R-EDIT-006 | Must |
| 5 | `undo-redo` | Ctrl+Z/Ctrl+Y undoes/redoes every editing operation shipped in slices 2–4, one interaction = one undo step, no silent data loss. | R-EDIT-008 | Must |
| 6 | `boolean-operations` | Union, difference, intersection on closed paths. | R-EDIT-003 | Must |
| 7 | `layers-and-grouping` | Group/ungroup objects; layers with per-layer visibility and lock, to separate cut/engrave/reference geometry. | R-EDIT-009 | Must |
| 8 | `svg-import-export` | Open a plain SVG from Inkscape and re-export it without hand-fixing geometry; named, listed loss report for anything outside our supported subset. | R-SYS-006 | Must |
| 9 | `raster-trace` | Trace a raster image to vector paths with adjustable threshold/color-count, parity with Inkscape's "Trace Bitmap". | R-VEC-001 | Must |
| 10 | `machine-profile` | Define and reuse a machine profile (work area, connection, limits) for a laser cutter; select it for a project. | R-MFG-001 | Must |
| 11 | `manufacturing-roles` | Assign cut/engrave role to geometry (by layer) within one file. | R-MFG-002 | Must |
| 12 | `material-test-library` | Generate a power/speed test-cut grid for the selected machine, record which cell worked, store it as a reusable material record. | R-MAT-001, R-MAT-002, R-MAT-003 | Must |
| 13 | `laser-job-preview-and-output` | Toolpath/time preview, an explicit machine+material gate before export, and GRBL G-code export for cut + engrave geometry. The MVP's capstone: a maker's own design goes from drawing to a file their laser runs. | R-MFG-003, R-MFG-LASER-001, R-SYS-008 | Must |

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
- **5 (undo) after 2–4, before 6–7**: undo is cross-cutting infrastructure
  (ADR 0002 §9, ADR 0004 §2 — peer-scoped, CRDT-backed) but "covers every
  editing operation" (R-EDIT-008) is only checkable once there is more than
  one operation to undo. Slices 6 and 7 each add an acceptance criterion
  that their new operation is undoable through the mechanism slice 5
  establishes, rather than repeating slice 5's work.
- **8 (SVG round-trip) after 4 and 7**: round-tripping a real Inkscape file
  is only a meaningful test once paths, primitives, styling, groups and
  layers all exist to round-trip. Earlier would mean testing against
  near-empty documents.
- **10–13 last and in that order**: a job needs a machine profile (10)
  before roles can target one (11), roles and a machine before a material
  test is worth running (12), and all three before the capstone job-preview
  export story (13) — which is where R-SYS-008's "no job without a machine
  profile" gate actually bites.

## Out of sequence (tracked in `docs/requirements.md`, not listed above)

Should/Could requirements, and every Must deferred past the laser MVP by
the customer's 2026-10-02 confirmation (multi-OS parity/sync, fonts, asset
management, collaboration, the asset connector, plugins, other machine
families) — see "MVP (confirmed)" and "Explicitly deferred past MVP" in
`docs/requirements.md`. These get their own slices, sequenced after #13, once
the customer prioritizes them.
