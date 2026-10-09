# System requirements

Scope: the whole application, cross-cutting constraints, and what every
component must respect. Component-level detail (specific file formats,
toolpath algorithms, per-machine protocols, UI layouts) belongs in each
component's own `requirements.md`, to be written once that component's
crates exist — do not invent that structure ahead of code.

Priority: **Must** (MVP-eligible), **Should** (expected before "comparable
to LightBurn"), **Could** (useful, not scheduled). Origin is **Customer**
(stated in the brief) or **Proposal** (this document's addition — needs
customer sign-off before it drives a story).

Every requirement has an id (`R-<area>-NNN`) so stories and ADRs can refer
to it instead of restating it.

---

## 1. Cross-cutting / platform

| id | requirement | priority | origin |
|---|---|---|---|
| R-SYS-001 | Runs as a native desktop app on Linux, Windows and macOS with one shared codebase. | Must | Customer |
| R-SYS-002 | All project data (drawings, material records, asset metadata) is readable and editable without a network connection. Nothing the maker owns is locked behind a server round-trip. | Must | Customer |
| R-SYS-003 | A project file opens correctly on Linux, Windows and macOS — same geometry, same units, same rendered result (within the tolerance defined by the document-model ADR). | Must | Proposal |
| R-SYS-004 | Projects and the material/asset library sync across the maker's own computers, and with collaborators' computers for shared projects. Cloud sync (§12) is the standing mechanism; folder-based sync (e.g. Nextcloud) is an additional offline-friendly option, not a replacement. | Must | Customer |
| R-SYS-005 | Runs in a browser with feature parity for drawing and job preparation; device communication may be unavailable where the browser has no device API, with export-to-file as the fallback. | Could | Customer (CLAUDE.md platform order) |
| R-SYS-006 | Imports and exports plain SVG well enough that a project can move to/from Inkscape without hand-fixing geometry, so a maker can switch tools gradually instead of in one jump. | Must | Proposal |
| R-SYS-007 | No telemetry (usage/analytics data about the maker) leaves the machine by default. Project data leaves the machine only for features the maker actively uses — real-time collaboration, cloud sync, or an asset-library connection (§12, §11) — and never silently as a side effect of an unrelated story. Where it leaves for collaboration/cloud sync, it is end-to-end encrypted per R-COLLAB-006. | Must | Proposal (flows from CLAUDE.md §3), reconciled with Customer decisions in §12 |
| R-SYS-008 | The app states clearly, in the UI, which machine/material combination a job will run with before the maker sends it, and refuses to send a job with no machine profile selected. | Must | Proposal |

## 2. Vector editing

| id | requirement | priority | origin |
|---|---|---|---|
| R-EDIT-001 | Create and edit Bézier paths by dragging nodes and handles, with the same mental model (node, handle, segment) makers already know from Inkscape. | Must | Customer |
| R-EDIT-002 | Primitive shapes: rectangle (with optional corner radius), circle/ellipse, and a general polygon/star tool, all editable as paths after creation. Shift+drag draws rectangle and ellipse from the center, Shift+Ctrl a square or circle from the center: `specs/0011-shape-creation-from-center/`. Primitives are edited in the one Select tool with their own parameter handles (draft): `specs/0009-unified-object-editing/`; a separate radius per rectangle corner (draft): `specs/0013-rectangle-corner-radii/`; arc handles on circle/ellipse and node shaping (draft): `specs/0021-ellipse-arcs-and-shaping/`. | Must | Customer |
| R-EDIT-003 | Boolean operations: union, difference, intersection on closed paths. Done (#69, #73); amended 2026-10-10: five operations (union, difference, intersection, exclusion, reverse difference) in their own toolbox card in the left tool rail, compound-path results with holes (document-model and file-format change approved by the customer), refusal of open paths and of empty results: `specs/0016-boolean-operations/`. | Must | Customer |
| R-EDIT-004 | "Object to path" converts any shape or text into an editable path. | Must | Customer |
| R-EDIT-005 | Stroke styling: width, dash pattern (presets and a typed custom pattern), join/cap style, color with alpha (8-digit RGBA hex, eyedropper). Panel behaviour after the customer's test: empty when nothing is selected, controls hidden while paint is None, inline controls with no popups, GIMP-style drag-or-type value fields: `specs/0017-style-panel-rework/`. | Must | Customer |
| R-EDIT-006 | Fill styling: none or solid color. **Gradients were part of this requirement and are removed (customer, 2026-10-08):** no application of ours needs them; if they come back it is as a new requirement with a new design. `specs/0017-style-panel-rework/` | Must | Customer |
| R-EDIT-007 | Outline/offset operation: produce an outset or inset copy of a path at a given distance, needed for kiss-cut borders, engrave-vs-cut separation, and seam allowances. The customer asked for Inset and Outset on 2026-10-09 (typed distance, live preview, corner joins, open and closed paths, holes); needs a kernel decision (ADR 0003 §4). `specs/0038-path-offset/` | Should | Customer (asked 2026-10-09; first a Proposal) |
| R-EDIT-008 | Undo/redo covers every editing operation above with no silent data loss. | Must | Proposal |
| R-EDIT-009 | Grouping, layers, and per-layer visibility/lock, matching what a maker needs to separate "cut", "engrave" and "reference" geometry in one file. Split on 2026-10-10: grouping (group, nested groups, enter and leave a group, ungroup; no group style or transform) is `specs/0023-groups/` (customer asked for it on 2026-10-09; needs a document-model decision); layers with visibility and lock are `0039` in `specs/README.md`, not yet specified. | Must | Proposal (in the customer-confirmed MVP; groups requested by the customer 2026-10-09) |
| R-EDIT-010 | Pan and zoom the canvas (scroll, drag, pinch), zooming toward the cursor, with the document's apparent scale and position staying stable when the window/viewport is resized. | Must | Customer |
| R-EDIT-011 | A general Select tool: click any existing object, regardless of which tool created it, to select it, then move or delete it without re-entering that object's own creation tool. | Must | Customer |
| R-EDIT-012 | Scale and rotate any selected object (path or primitive) via on-canvas transform handles, matching the resize/rotate handle convention makers already know from Inkscape, Illustrator or Figma — not just the plain move R-EDIT-011 already covers. Refinements after the customer's first test (center move handle, rotate handles at corners and sides, pivot rule, typed angle/size entry, 22.5° snap stop): `specs/0008-object-transform-refinements/`. Unification with primitive editing, one edit mode for every object and the blue-new/black-old preview in every edit (draft): `specs/0009-unified-object-editing/`. Second change list after testing that unification (draft): deterministic angle when creating a polygon or star, typed skew and typed move (relative or absolute) by double-click, Ctrl-copy and Shift-axis-lock while moving, Escape back to the Select tool in every tool, a single selected node after Split, and a dashed-selection-box design experiment: `specs/0010-edit-interaction-polish/`. | Must | Customer |
| R-EDIT-013 | While the node tool is active, an option to show the handles of every node on the selected path at once (not just the currently-selected node/handle, which is `path-node-editing`'s (slice 2, Done) existing behaviour per its AC7 and UX notes), and hide all of them again automatically when the node tool is switched off. Likely an Inkscape-style toggle (Inkscape: "Show next/previous path effect parameter" aside — the relevant precedent is Inkscape's node tool showing all handles for all nodes of a selected path by default, with no per-node hiding at all). Needs its own thin slice: decide the toggle's UI location (toolbar button vs. tool-controls-bar button, see slice 2's contextual tool-controls bar), its default state, and whether it is per-path or a global node-tool preference, before writing acceptance criteria. | Should | Customer |
| R-EDIT-014 | Merge two open path endpoints into one continuous path, and split a path at a selected node into two (Inkscape's "join selected endnodes" / "break path at node"); a third node type, tangent-continuous with independently adjustable handle lengths (Inkscape calls this "smooth" — this product's existing `Smooth` kind is renamed `Symmetric` to free the name), alongside the existing `Corner`/`Symmetric` pair from `path-node-editing`. | Must | Customer |
| R-EDIT-015 | On-canvas skew (shear) arrow handles in x and y, Inkscape-style (parallelogram), on a selected path, mixing Inkscape's and LightBurn's skew handles. **Unblocked (customer, 2026-10-06).** Paths (open and closed) only: the skew is baked into anchors and handle vectors, no document-model change. Primitives (rectangle, ellipse, polygon, star) are excluded: no skew handles, nothing happens (the maker uses "Object to path" first); keeping primitives skewable needs a stored per-object matrix, deferred until the primitives rework. Taper (true trapezoid) is not included. Criteria 37 onward in `specs/0008-object-transform-refinements/` Part B. | Should | Customer |
| R-EDIT-016 | Stroke markers: an arrow or a thick dot at the start, at the end, at a settable number of evenly spaced places along (default 1), or on every node of a path; part of the stroke style, in the stroke colour. `specs/0018-stroke-markers/` | Should | Customer |
| R-EDIT-017 | Color management: customizable and extendable colour palettes, thread palettes for embroidery, colour history, current/old swatch, other colour models (HSL, HSV, CMYK) as one advanced feature, inline in the properties panel, not a wide bar. After the MVP. Sketch: `specs/0022-color-management/` | Could | Customer |
| R-EDIT-018 | Document size and rulers: set the single document's width and height (the centre stays fixed), fit it to the drawing, rulers along the top and left of the canvas, drawing outside the document on a pasteboard; display unit mm/cm/in is a proposal. Scheduled by the customer on 2026-10-09 ahead of `undo-redo`; not in the MVP list below, which stays as confirmed. Several pages were dropped (2026-10-05). Done (#67, #72): `specs/0015-document-size-and-rulers/`. | Must | Customer |
| R-EDIT-019 | Document size presets: ready-made document formats DIN A0 to A6 and slide formats 16:9, 16:10 and 4:3 (pixel sizes at 96 per inch, stored in mm), chosen with inline buttons in the Document section, a portrait/landscape switch, the selected format derived from the size; the list is one validated data file so adding a format needs no code. User-defined presets later. `specs/0030-document-size-presets/` | Should | Customer |
| R-EDIT-020 | Segment bending: in the Node tool, drag a line or curve segment of a path to bend it (the segment's two handles move so the curve follows the pointer), honouring node types, with a blue/black preview, one commit and Escape to cancel. `specs/0031-segment-drag-bending/` | Should | Customer |
| R-EDIT-021 | Stroke brushes: a stroke can be drawn by a brush (width profile, stamp, pattern along the path), chosen inline in the Stroke section, customizable; custom brushes are added by the maker as data files, or later as plugins; brush strokes are expanded to paths for jobs. After the MVP. Sketch: `specs/0033-stroke-brushes/` | Could | Customer |
| R-EDIT-022 | Pen path extension: with the Pen, continue an existing open path from either end, connect two paths by drawing from one end onto the other, choose a sharp or smooth closing node when closing a path, and close a selected open path with a command. `specs/0034-pen-path-extension/` | Should | Customer |
| R-EDIT-023 | Path operations beyond booleans, as plain commands: Combine and Break apart compound paths (holes kept), Split paths at their crossings, Fracture overlapping shapes into pieces, Flatten a stack to its visible parts. `specs/0035-combine-and-break-apart/`, `specs/0036-split-at-crossings/`, `specs/0037-fracture-and-flatten/` | Should | Customer |

## 3. Vectorization (raster-to-vector)

| id | requirement | priority | origin |
|---|---|---|---|
| R-VEC-001 | Trace a raster image to vector paths with adjustable threshold/color-count settings, comparable to Inkscape's "Trace Bitmap". | Must | Customer |
| R-VEC-002 | Combine multiple trace passes (e.g. different thresholds or color layers) into one result, so fine detail and large flat areas can each use their own best setting instead of one compromise setting for the whole image. | Should | Customer |
| R-VEC-003 | Path simplification that reduces node count while holding a stated maximum deviation from the original trace, and visibly outperforms Inkscape's simplify on a corner-heavy test image (fewer nodes at the same deviation tolerance). | Should | Customer |
| R-VEC-004 | Live preview of trace settings on the actual source image before committing, so a maker doesn't trace-undo-retrace repeatedly. | Could | Proposal |

## 4. Manufacturing — general

| id | requirement | priority | origin |
|---|---|---|---|
| R-MFG-001 | Define a machine profile per physical device (machine family, work area, connection, machine-specific limits) and reuse it across projects. | Must | Customer |
| R-MFG-002 | Assign manufacturing roles to geometry (e.g. cut / engrave / score / stitch-fill) within one file, so one drawing can drive one job with mixed operations. | Must | Proposal |
| R-MFG-003 | Generate a job preview (toolpath/stitch preview, estimated time) before sending to the machine. | Must | Proposal |
| R-MFG-004 | Nesting/arranging multiple parts on the machine's work area, with basic rotation to reduce material waste. | Should | Proposal |

## 5. Manufacturing — laser cutter/engraver

| id | requirement | priority | origin |
|---|---|---|---|
| R-MFG-LASER-001 | Generate laser-ready output (vector cut paths with power/speed, and raster engrave fills) for at least one common controller dialect (e.g. GRBL G-code). | Must | Customer |
| R-MFG-LASER-002 | Send a job directly to a connected laser over at least one transport (serial or network), not just file export, matching what LightBurn already does today. | Should | Customer |
| R-MFG-LASER-003 | Live job control: start, pause, stop, and frame (outline the job at low power to check placement) on a connected machine. | Should | Customer |

## 6. Manufacturing — cutting plotter

| id | requirement | priority | origin |
|---|---|---|---|
| R-MFG-CUT-001 | Generate HPGL (or the plotter's native command set) output for cut and draw/pen operations from the same vector file used for other machines. | Must | Customer |
| R-MFG-CUT-002 | Support registration-mark-based alignment for print-then-cut workflows. | Could | Proposal |

## 7. Manufacturing — embroidery

| id | requirement | priority | origin |
|---|---|---|---|
| R-MFG-EMB-001 | Convert vector fills/strokes into embroidery stitch paths (at minimum running stitch and satin/fill) comparable to Ink/Stitch's core conversion. | Should | Customer |
| R-MFG-EMB-002 | Export at least one common embroidery machine format (e.g. DST). | Should | Customer |
| R-MFG-EMB-003 | Stitch simulation/preview before export. | Should | Proposal |

## 8. Manufacturing — CNC mill

| id | requirement | priority | origin |
|---|---|---|---|
| R-MFG-CNC-001 | Generate 2.5D toolpaths (profile, pocket, drill) from vector geometry with configurable tool diameter and depth. | Should | Customer |
| R-MFG-CNC-002 | V-carve text and vector art using a Voronoi-diagram-based approach (the brief names OpenVoronoi as a reference), including choice of V-bit angle and max depth. | Should | Customer |
| R-MFG-CNC-003 | Export standard G-code for 2.5D/V-carve jobs. | Should | Customer |

## 9. Material management

| id | requirement | priority | origin |
|---|---|---|---|
| R-MAT-001 | Store a material record (name, thickness, machine, settings) per machine/material combination. | Must | Customer |
| R-MAT-002 | Generate and run a standard test-cut/engrave pattern (a settings grid: power x speed, or the material-appropriate equivalent) for a chosen machine. | Must | Customer |
| R-MAT-003 | Record the result of a test pattern (which cell worked) and attach it to the material record for reuse in future jobs on the same machine. | Must | Customer |
| R-MAT-004 | Material records are per-machine but a maker can view/compare records for the same material across machines/materials. | Should | Proposal |
| R-MAT-005 | Material records sync across the maker's computers (same mechanism as R-SYS-004). | Should | Customer |

## 10. Font management

| id | requirement | priority | origin |
|---|---|---|---|
| R-FONT-001 | Use a font in a project without installing it system-wide (project-local or library-local font storage, loaded only into this app). | Must | Customer |
| R-FONT-002 | Categorize fonts by manufacturing suitability: single-line/connected-glyph (cuttable in one pass), initials/monograms, symbols/ornaments, and general-purpose, with a custom-tag option. | Should | Customer |
| R-FONT-003 | Filter/hide fonts not relevant to the current project (e.g. show only "cuttable" fonts while preparing a laser job) without affecting any other application's font list. | Should | Customer |
| R-FONT-004 | Font categorization works on a curated/supplied set out of the box; a maker can recategorize any font. | Could | Proposal |

**Tracked, not blocking (open research item):** unlike icons/symbols (§11,
Iconify), no third-party asset-library service for *fonts* has been
identified yet. The customer confirmed this is unresolved ("für Fonts müssen
wir uns noch etwas bauen — ich habe noch nichts Brauchbares gefunden") and
that it stays an open research item, to be revisited once the
asset-connector slice (R-ASSET-004/005) is underway, rather than a blocker
on anything sequenced before it. Font requirements are already deferred past
MVP, so nothing in the current plan is waiting on this. It means
R-ASSET-004/005 cannot be assumed to cover font sourcing, and a
font-specific connector or source needs its own research/decision before any
story is written against it.

## 11. Asset management

| id | requirement | priority | origin |
|---|---|---|---|
| R-ASSET-001 | Save a design (or part of one) as a reusable template/asset with a thumbnail and searchable name/tags. | Must | Customer |
| R-ASSET-002 | Browse and insert assets into the current project without leaving the app. | Must | Customer |
| R-ASSET-003 | Add additional asset libraries beyond the built-in one (e.g. a second folder/source a maker points the app at). | Should | Customer |
| R-ASSET-004 | Connect to a third-party asset-library service using credentials the maker supplies (API token, or username + password) to browse, search and insert that provider's assets alongside the built-in library. We do not build or operate accounts, payments or subscriptions ourselves — that is entirely the asset provider's responsibility; we only consume their API with maker-supplied credentials. This is the product's primary monetization lever; every other requirement in this document is deliberately free. Credential storage is a security-sensitive design question covered by the architect's ADR, not by this requirement. The first concrete connector target is [Iconify](https://iconify.design) (a curated, mostly read-oriented open icon-set aggregator), serving the symbol/ornament asset case and tying into R-FONT-002/003's symbol/ornament category. | Must | Customer |
| R-ASSET-005 | Where the maker's configured credential for a connected third-party asset-library service grants upload/push permission, upload the maker's own drawings or exports to that service directly from the app, in addition to browsing/downloading its assets (R-ASSET-004). The upload target is always a third-party service the maker already has write access to via their own credential — this is not our own hosted storage, account system or payment path, and it does not change R-ASSET-004's "we don't run accounts or billing" boundary. The first concrete read+write target is a **git forge** (e.g. pushing assets/exports into a repository the maker already has write access to via their own git-forge account) — chosen by the customer over WebDAV. Iconify (R-ASSET-004) stays the first read-oriented target and is not in scope for this requirement; the requirement itself is connector-capability-level, not tied to one provider. | Must | Customer |

## 12. Real-time collaboration and sync

| id | requirement | priority | origin |
|---|---|---|---|
| R-COLLAB-001 | Two or more makers, each on their own instance, can edit the same document at the same time and see each other's changes live (continuous co-editing, not file locking or merge-after-the-fact), comparable in spirit to how Zed supports collaborative text editing but for vector documents. | Must | Customer |
| R-COLLAB-002 | A server component coordinates real-time collaboration sessions between instances. Whether it is operator-hosted, self-hostable, or both is an architecture decision (see the architect's ADR); this requirement only states that live co-editing needs a server and that the server is part of the product, not a third-party dependency the maker must assemble. | Must | Customer |
| R-COLLAB-003 | Cloud sync of projects and the material/asset library is a standing, always-available capability — not a "later" feature and not conditional on the maker setting up their own sync folder. | Must | Customer |
| R-COLLAB-004 | Folder-based sync (e.g. a Nextcloud-synced folder, or any other synced-folder tool) is available as an additional, offline-friendly sync option alongside cloud sync, for makers who prefer not to rely on a hosted service for this. | Should | Customer |
| R-COLLAB-005 | With no network connection, a maker can still open, edit, save and export their own local project files with full single-user functionality; only live collaboration and cloud sync are unavailable until connectivity returns. | Must | Proposal (reconciles R-COLLAB-001 – 003 with R-SYS-002) |
| R-COLLAB-006 | Document content handled by the collaboration/sync server is encrypted end-to-end: the server operator — including a self-hoster who is not one of the document's own collaborators — cannot read project/document content, under current (2026) state-of-the-art practice for end-to-end encrypted collaboration/sync systems. Transport security (TLS) and server-side encryption-at-rest do not satisfy this on their own; only a maker holding (or granted) the right key material can decrypt content. This applies to both real-time collaboration (R-COLLAB-001/002) and cloud sync (R-COLLAB-003). The key-management/crypto approach is the architect's design; this requirement only states the capability the maker gets. | Must | Customer |

### Collaboration lifecycle (use cases)

The customer confirmed collaborator removal is required, wants the concept
worked out as explicit flows before the technical revocation mechanism is
chosen, and accepts that "removed" means the removed person keeps the last
state they had locally but gets nothing further — not that their copy is
remotely wiped. The architect's ADR covers *how* a key is revoked/rotated;
these requirements state only the capability a maker-facing flow must
provide.

| id | requirement | priority | origin |
|---|---|---|---|
| R-COLLAB-007 | A maker (any admin of a shared document, see R-COLLAB-012) can invite another maker as a collaborator on that document, without either party handling raw cryptographic key material by hand. | Must | Customer |
| R-COLLAB-008 | An invited maker can accept an invitation and, from that point on, co-edit the shared document live with the same capability as R-COLLAB-001. | Must | Customer |
| R-COLLAB-009 | A collaborator can leave a shared document voluntarily; after leaving, they stop receiving further updates and can no longer push changes to it, but the project file they already had stays usable on their own machine (R-SYS-002 still applies to it). | Should | Customer |
| R-COLLAB-010 | An admin can remove another collaborator from a shared document. After removal, the removed person receives no further updates and cannot push new changes, but keeps the last state they had already synced locally — removal never deletes or locks their existing local copy. | Must | Customer |
| R-COLLAB-011 | An admin can re-add a previously removed collaborator to a shared document, restoring their ability to receive and push updates for it. | Should | Customer |
| R-COLLAB-012 | A shared document can have more than one admin at the same time, so invite/remove/re-add capability does not depend on one specific person's availability. | Must | Customer |
| R-COLLAB-013 | If every admin of a shared document has lost access to their key material and no recovery key is available, no one can invite, remove or re-add collaborators on that document going forward; the only remaining path to its content is a surviving collaborator's local, unencrypted copy of the project file on their own filesystem. This is an acknowledged hard limit of a decentralized, key-based system, not a supported recovery feature. | Must | Customer |

## 13. Extensibility (plugins)

| id | requirement | priority | origin |
|---|---|---|---|
| R-EXT-001 | The product exposes a plugin interface — written in Rust, built against the same core crates the application itself uses — for extending drawing tools, vectorization passes, manufacturing/output generation, and asset-library connectors. Plugins are a core, load-bearing part of the architecture from the start, not an optional add-on bolted on after the fact. | Must | Customer |
| R-EXT-002 | Installing, updating, enabling and disabling a plugin happens inside the app (browse/search a plugin list, one-click install, no hand-edited config files or restart into a separate extension manager), directly addressing the "Inkscape's extensibility is slow and bolted-on" pain point named in the brief. | Should | Proposal |

## 14. Input devices

| id | requirement | priority | origin |
|---|---|---|---|
| R-INP-001 | Pen tablet input: pressure (and tilt, and the eraser end) from a drawing tablet changes the width of freehand strokes and erases objects, on desktop first, with a mouse unaffected. "Later, not now" (customer, 2026-10-09). Needs a freehand drawing tool first, a per-platform check that the webview delivers pressure (Linux in particular), and a document-model decision on how a variable width is stored. Sketch: `specs/0032-pen-tablet-input/` | Could | Customer |

R-EDIT-019 to R-EDIT-023 and R-INP-001 are not in the MVP list below, which stays as confirmed; the customer schedules them (build order proposal in `specs/README.md`).

---

## MVP (confirmed)

The MVP is the thinnest slice that replaces the maker's single most urgent
broken tool (LightBurn, Linux-dead as of 1.8) with something that works end
to end for **one machine family, one material, one real job**, while giving
back enough drawing power that the maker is not forced into Inkscape first.
Everything else in this document is sequenced after it.

The customer confirmed this cut (2026-10-02): laser-only, local-only, one
controller dialect, cutting plotter not pulled into the first slice.

**MVP = these Musts, and nothing else:**

- R-SYS-001, R-SYS-002, R-SYS-006, R-SYS-007, R-SYS-008 (desktop, offline,
  SVG interop with Inkscape, no telemetry, explicit machine/material gate)
- R-EDIT-001 – R-EDIT-006, R-EDIT-008 – R-EDIT-011 (drawing: paths,
  primitives, booleans, object-to-path, stroke/fill, undo, layers, canvas
  navigation, general selection)
- R-VEC-001 (basic raster trace, parity with Inkscape's current feature)
- R-MFG-001, R-MFG-002, R-MFG-003 (machine profile, cut/engrave roles, job
  preview)
- R-MFG-LASER-001 (laser G-code output for cut + engrave)
- R-MAT-001, R-MAT-002, R-MAT-003 (material record, test pattern,
  recorded result — **local only**, no sync yet)

**Explicitly deferred past MVP** (Must-priority but not in the first
slice, because they widen scope rather than deepen the first job):
R-SYS-003/004 (multi-OS file parity and sync — matters once there's a
second machine/computer to prove it against), R-FONT-001, R-ASSET-001/002
(font and asset management — real pain points, but the brief's most acute
problem is "LightBurn is dead on Linux and knows nothing about materials",
not fonts or assets), R-COLLAB-001/002/003/006 (real-time collaboration, its
server, and the end-to-end encryption that server must provide), R-COLLAB-
007–013 (the invite/leave/remove/re-add/multi-admin/worst-case lifecycle
around that same collaboration capability), R-ASSET-004/005 (asset-library
connector, read and write), R-EXT-001 (plugin interface) — see the
MVP-sequencing note below for why these Musts are deferred as *features*
while still shaping the architecture now.

Cutting plotter, embroidery, CNC and the Should/Could manufacturing rows are
each their own later slice once the laser slice is proven, in the order the
customer prioritizes.

### MVP-sequencing note: architecture-ready vs feature-complete

R-COLLAB-001–003/006 (real-time co-editing + server + cloud sync, end-to-end
encrypted), R-COLLAB-007–013 (invite/leave/remove/re-add/admin lifecycle
around that collaboration), R-ASSET-004/005 (asset-library connector, read
and write) and
R-EXT-001 (Rust plugin interface) are all Must-priority, and the customer
has named two of them ("plugins are an essential component", "the
asset-library connector is the only thing that can make money") as central
to the product, not peripheral. That does **not** mean the first shippable
slice needs working real-time collaboration or a working connector.
Shipping those as working features *before* the laser-cutting MVP proves
itself would turn a thin, provable first slice into a
distributed-systems-cryptography-and-payments-adjacent project with no user
yet.

**Confirmed (2026-10-02):** the laser MVP's architecture is built
*collaboration-ready, connector-ready and plugin-ready* — i.e. the document
model, the sync/storage layer, the credential-handling boundary, and the
plugin API surface are designed up front (architect's ADRs
0004/0005/0006/0007) so that adding real-time collaboration, the asset
connector, and the first real plugin later are additive slices, not
rewrites — but the laser MVP itself ships **single-user, local-only, with
no live plugin loaded and no asset connector wired up**. Each of the three
then becomes its own story immediately after the laser MVP is proven, in an
order the customer prioritizes.

---

## Resolved questions (2026-10-02)

The customer did not object to any of the following defaults, so they are
settled and now reflected as such throughout this document:

1. MVP cut: laser-only, local-only, one controller dialect. Cutting plotter
   stays out of the first slice. (See "MVP (confirmed)" above.)
2. MVP-sequencing: the laser MVP ships single-user/local-only; real-time
   collaboration, the asset-library connector, and the plugin interface are
   built architecture-ready but ship as features in later slices. (See the
   MVP-sequencing note above.)
3. Font sourcing: stays an open research item (§10), revisited once the
   asset-connector slice is underway. Not a blocker on anything currently
   planned.

No other open questions originate from this document at this time. New
questions, if any arise, will be added below as they come up.
