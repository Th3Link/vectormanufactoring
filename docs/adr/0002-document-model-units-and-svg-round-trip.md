# ADR 0002: Internal document model, units and SVG round-trip

**Status:** Accepted (customer sign-off, 2026-10-02); §10 amended 2026-10-08 (gradients removed from the modelled SVG subset)

Reconciled with [ADR 0004](0004-persistence-and-cross-machine-sync.md) (an open
document is a Loro CRDT replica) and
[ADR 0008](0008-end-to-end-encryption-of-sync-and-collaboration.md) (updates are
sealed client-side before they leave the process), where the first draft had
assumed a single writer and a local-only document. Changed: **§5**, identity and
sibling order come from the CRDT rather than an arena, and **§9**, the operation
log is the canonical history with the command journal as the edit API over it.
Units, Y-down, curves, manufacturing intent, the SVG subset and versioning
stand, with replication's obligations noted inline. The *behavioural*
consequences — undo scope, ephemeral state, merge granularity — are
[ADR 0009](0009-concurrent-editing-semantics.md).

## Context

The document model is the single most expensive thing in this project to
change later: every tool, every importer and exporter, every toolpath
generator and the project file format all depend on it. It has to serve two
masters that pull in different directions — SVG-shaped vector editing on one
side, millimetre-exact manufacturing on the other — and it has to stay pure
and wasm-compatible (`CLAUDE.md` §6).

It is also a **replicated** model, so every structure here carries a second
question — what happens when two peers change it at once — and ADR 0008 adds a
third: everything leaving the process is sealed, so no server can ever merge,
repair or migrate a document on our behalf. The merge behaviour of a field is
therefore part of the document model, not a detail a sync layer can decide
later. This ADR fixes the *shape*; ADR 0009 fixes what concurrent edits to that
shape mean.

### Options considered

**A. Use an SVG DOM as the in-memory model** (Inkscape's approach: the file
format *is* the model). Round-trip fidelity is perfect by construction.
Rejected: it imports SVG's entire accidental complexity — the CSS cascade,
nested transform/viewBox unit contexts, `use` clones, the SVG text model —
into every part of the application, including toolpath generation, where none
of it means anything. Manufacturing metadata (job assignment, power, speed,
stitch parameters, tool and depth) has no natural home and would live in a
private namespace. Geometry operations would fight the DOM on every call.

**B. Our own document model; SVG is an import/export format.** Clean domain
types, units as newtypes, manufacturing data first class, cheap geometry.
Cost: round-trip is lossy unless managed deliberately.

**C. Our own model plus the original SVG kept alongside, re-emitted by
patching the original on export.** Best possible fidelity for
"import, change one thing, export". Rejected as a day-one choice: two sources
of truth that must be kept in agreement is a defect generator, and the
customer's workflow is "own this document", not "be a careful SVG filter".
The passthrough bag in B covers most of the practical benefit.

### Unit representation, considered separately

Coordinates as `f64` millimetres, versus fixed-point integers (e.g. 1/1000 mm,
as Clipper2-style robust polygon code wants), versus a unit-generic model
(`uom`/typenum). Integers give exact predicates and robust booleans but make
Bézier math, offsetting and glyph outlines awkward. A unit-generic model
violates `CLAUDE.md` §5 ("no generic parameter unless two concrete types use
it now") and buys nothing: there is exactly one internal unit, and mm/cm/inch
are a presentation concern.

### Which log is the document's history — the question ADR 0004 left open

The first draft made the command journal the persisted history and the undo
mechanism. ADR 0004 §2 then made the document a CRDT replica, which gives it a
second, independent history. Two histories of one document cannot both be
canonical.

**1. Journal canonical, CRDT state replayed from it on open.** Rejected:
replaying a linear journal mints *new* operation ids, so a reopened document no
longer converges with a peer holding the originals, and two peers replaying
their own journals diverge permanently. Rebasing one journal against a
concurrent one is the operational transform ADR 0004 rejected as option D,
reached by a nicer-looking route.

**2. Both canonical — journal for undo with recorded inverses, CRDT for sync.**
Rejected, and this is a data-loss bug rather than an untidiness: an inverse
captured when the command was authored ("stroke width 2 → 1") assumes a state
that no longer exists once a collaborator has written 5, and applying it
discards their edit. Inverses have to be derived from the live log against
current state, which is the CRDT's job.

**3. Operation log canonical; the journal becomes the edit API and the
undo/history surface over it.** Chosen. One history, one merge implementation,
and the journal keeps the three jobs it is good at: the single funnel every edit
passes through (ADR 0005 §5 depends on it), serializable intent for tests and
scripting, and a place to hang a label and an interaction boundary so undo has
sensible granularity.

## Decision

1. **Own document model** in `vecmanf-document-core`, pure, wasm-compatible.
   SVG is one importer/exporter among several, with golden-file tests
   (`CLAUDE.md` §5). The model is defined by our own types; its CRDT backing is
   internal to the crate and appears in no public API (ADR 0004 §3).
2. **Canonical unit is the millimetre**, stored as `f64` in document space.
   All four machine families speak mm natively (metric G-code, HPGL at 1/40 mm
   plotter units, embroidery stitch formats at 0.1 mm), as do material sizes.
   SVG's 96 dpi user units are converted at the import/export boundary, once.
3. **Units are newtypes**, never bare `f64`: `Length` (mm), `Point`, `Vec2`,
   `Angle` (radians), `Speed` (mm/s), `Power` (normalized 0.0–1.0 plus the
   machine's own scale at export), `Frequency` (Hz), `Dpi`, `Tolerance`
   (a `Length`). Every geometric comparison takes an explicit `Tolerance`;
   there is no global epsilon.
4. **Document Y grows downward**, matching SVG and screen space. The flip to
   Y-up happens once, at job generation per machine, where it is a single
   tested transform — rather than at every import, render and hit test.
5. **Structure: a tree whose node identity and sibling order come from the
   CRDT.** `NodeId` is a globally unique id minted by the creating peer — never
   an array index, arena slot or generational handle, none of which survive
   replication — stable for the document's lifetime, and what selections,
   history entries and external references use. Sibling order *is* z-order and
   is the CRDT's own ordering; nothing iterates children in local insertion
   order, so every peer and every export sees the same order. Reparenting and
   "bring to front" are tree-move operations, the one reason Loro was chosen
   over Automerge (ADR 0004). Each node carries its own affine transform and
   its own fully resolved style: no CSS cascade, no inheritance — which under
   replication also means no shared parent register for concurrent style edits
   to contend on. Named styles are a later story. Reading the CRDT tree
   directly is the baseline; a derived local read model for rendering and
   hit-testing is permitted when measurement asks for one, and is never
   authoritative.
6. **Curves:** paths are sequences of cubic Bézier segments and lines, with
   explicit open/closed subpaths. Rectangles, circles and ellipses are kept as
   their own primitives (they carry editing intent and corner radii);
   "object to path" is an explicit, destructive conversion. How a path's
   anchors merge under concurrent editing is ADR 0009 §3.
7. **Text:** a text node references a font by a stable font identity (family,
   style, and a content hash of the face) plus its string and layout; glyph
   outlines are materialized on demand and never stored in the document — which
   replication makes doubly true, since stored outlines would put a megabyte of
   geometry into the operation log for every text edit. Glyph geometry is
   modelled as **paths that may be open**, so single-line / engraving /
   cuttable fonts work the same way filled outlines do. This is a deliberate
   early call: it is what makes plotter, laser-engrave and embroidery text
   possible at all. The string merges per character (ADR 0009 §3); a peer that
   does not hold the face is ADR 0009 §5.
8. **Manufacturing intent lives in the document**: nodes are assigned to
   layers, and a layer carries a job assignment that *references* a machine
   and material profile by UUID (ADR 0004). A generated job additionally
   **embeds a snapshot** of the resolved parameters, so reopening an old
   project reproduces the output even after the material database changes. The
   snapshot is written once and never edited, so it needs no merge rule — and
   it gains a second purpose under collaboration: the machine/material database
   is deliberately not replicated (ADR 0004 §6), so a collaborator may not hold
   the referenced UUID at all, and the embedded snapshot is what makes the job
   readable to them.
9. **The CRDT operation log is the document's history; the command journal is
   the edit API and the undo/history surface over it.**
   - Every edit is a serializable `Command` and no edit path bypasses it
     (ADR 0005 §5 depends on this). A command **no longer carries an inverse**:
     applying one translates it into CRDT operations in a single commit, tagged
     with the local peer id and a human-readable label.
   - That commit is the unit of undo and one row of the history view. An
     interaction spanning many input events — a node drag — is **one** commit,
     made when the interaction completes; ADR 0009 §2 covers what collaborators
     see while it is in flight, and ADR 0009 §1 what undo then does.
   - The journal is not persisted and not replicated. `document.loro` holds the
     operation log (ADR 0004 §1); the journal lives in memory for the open
     session only.
10. **SVG round-trip strategy:** we own a documented subset (paths,
    primitives, groups, transforms, fills and strokes including dash patterns,
    linear/radial gradients, clip paths, images, text). Anything we parse but
    do not model is preserved verbatim in a per-node **passthrough bag** and
    re-emitted on export. Anything we cannot parse at all is reported to the
    importing user as a named, listed loss — never dropped silently. Round-trip
    behaviour is pinned by golden files in `tests/fixtures/`. Export reads a
    version snapshot, not live state (ADR 0009 §4), which is what keeps the
    golden files meaningful under collaboration.

    Amended 2026-10-08 (customer decision, YAGNI): **gradients are not part of
    the modelled subset.** The document model carries no gradient paint; a
    fill is none or a solid color (R-EDIT-006). The gradient fill that
    `0007-stroke-and-fill-styling` added is removed by `style-panel-rework`,
    which also says what happens to saved documents that contain one. SVG import treats gradient paint as
    unsupported; what the importer does with it (substitute, passthrough or
    reported loss) is decided in the SVG import story. Gradients may return
    later as a new requirement with a new design, not by reviving this list
    entry. Follows from [`specs/style-panel-rework/`](../../specs/style-panel-rework/).
11. **The document is versioned** from the first commit (`format_version` plus
    a written migration policy, see ADR 0004 §9, which extends this to the Loro
    snapshot version).

## Consequences

- Every geometry and manufacturing operation reads a model built for it; no
  SVG semantics leak past the importer.
- Imported SVG does not come back out byte-identical, and CSS-heavy or
  text-heavy files from other tools will round-trip imperfectly. The
  passthrough bag plus an explicit loss report is the mitigation; this is
  recorded in `docs/technical-debt.md`.
- `f64` mm means boolean operations and other predicates need explicit
  tolerances and, inside the geometry kernel, possibly an internal scaled
  integer representation (ADR 0003). The document does not know about that.
- Resolved per-node styles mean a future "change all red strokes" or named
  styles feature is a real refactor, not a config switch. Under replication a
  bulk restyle is also one operation per affected node, so it adds log growth
  proportional to the selection. Accepted: it buys a model with no cascade to
  debug and no contended shared register.
- Open-path glyphs cost extra care in fill and boolean code (an open path has
  no inside), but there is no retrofit that would add single-line font support
  later without touching every text code path.
- Dropping command inverses removes code rather than adding it, and moves that
  correctness burden onto the CRDT's undo remapping: one more place where
  Loro's behaviour is effectively ours. Contained by ADR 0004 §3, recorded
  under the Loro-maturity entry in `docs/technical-debt.md`.
- `NodeId` is no longer an index into anything, so nothing may assume ids are
  dense, ordered or usable as array offsets. That is a constraint on every
  consumer of the model, including the renderer's draw list (ADR 0001 §4).
