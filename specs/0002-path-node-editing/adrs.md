# ADRs for "Path and node editing: draw and edit a Bézier path with a pen tool"

This slice is the first thing to put **geometry** into the document model: the
first path node, the first anchors, the first per-node style, and the first
canvas that draws and hit-tests. It reopens no accepted decision. It does make
four format-shaped choices that no ADR settles at anchor level, and those are
the dated feature-local decisions below.

All 14 acceptance criteria are buildable against ADR 0002 §5/§6 plus
ADR 0009 §3 **as those read today** — no array-of-points model, no positional
node index, no new merge rule. The reconciliation that makes that true is
spelled out under ADR 0002 and ADR 0009 rather than assumed.

## Depends on

- [ADR 0002 §5, §6](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  a path is **one tree node** with a CRDT-minted `NodeId`, carrying its own
  transform and its own fully resolved style (§5), whose geometry is cubic
  Bézier segments and lines with an explicit open/closed flag (§6). Two
  orderings meet here and must not be conflated: **sibling order among tree
  nodes is z-order** (§5), and **anchor order within one path is traversal
  order** (ADR 0009 §3's movable list). Both come from the CRDT; neither is an
  array index. §5's "nothing may assume ids are dense, ordered or usable as
  array offsets" binds anchors too.
- [ADR 0002 §2, §3, §4](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  every coordinate this slice writes is a millimetre `f64` behind a newtype —
  `Point` for an anchor, `Vec2` for a handle, `Length` for AC 6's 0.25 mm
  stroke — in Y-down document space, and every geometric comparison (segment
  and node hit-testing, AC 7/12/14) takes an explicit `Tolerance`. There is no
  global epsilon and no bare `f64` on a public signature.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md)
  and [ADR 0005 §5](../../docs/adr/0005-extension-and-plugin-model.md): every
  edit in this slice is a serializable `Command` through the journal — no tool
  code writes Loro directly — and **one interaction is one commit, made when
  the interaction completes**. This is load-bearing for AC 3/AC 4, not a
  formality: see the "a pen session is one commit" decision below.
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md): the
  merge rules this slice's operations need already exist and are **sufficient
  without amendment**. A path's anchors are a movable list, so insert (AC 12)
  and delete (AC 13) are list operations and two peers editing different
  anchors both keep their work; everything else an anchor carries — position,
  each handle, corner/smooth kind — plus `closed` and the style fields are
  per-field last-writer-wins registers. Two peers dragging the same node or
  the same handle therefore produce **one of the two values, never a blend**,
  which is §3's stated floor and option D's stated refusal, not a gap.
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): the
  in-progress pen path, the selected nodes/handles/segment (AC 7, 10, 14), the
  hover state and the in-flight geometry of every drag (AC 8, 9, 10) are
  **ephemeral state, not document state** — never operations, never in a saved
  file. Local state holds `AnchorId`s and resolves them lazily, dropping
  missing ones, so AC 13's delete (and a collaborator's) cannot leave a
  dangling selection.
- [ADR 0011 §2, §3](../../docs/adr/0011-workspace-and-crate-layout.md): the
  crate boundary for this slice, in the direction §3's edge list already
  fixes. See "the path/node crate boundary" below. **No crate is needed that
  ADR 0011 §1 does not already name**; four of its named-but-not-yet-created
  crates are stood up here (`curvyo-geometry-core`, `curvyo-render-core`,
  `curvyo-ui-core`, `curvyo-editor-wasm`).
- [ADR 0003 §1, §2, §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  `curvyo-geometry-core` is required by this slice, and for the reason
  ADR 0011 §3 already names — **hit-testing**. Deciding whether a click landed
  on a curved segment (AC 12, AC 14) means flattening that curve to a
  tolerance, which is §2's `kurbo`, in §1's kernel. Its contents here are
  three narrow things: flatten-for-hit-test, nearest-point-on-segment
  (returning the parameter), and subdivide-at-parameter for AC 12. §7's
  separate, coarser display tolerance in `curvyo-render-core` must not be
  reused for any of them.
- [ADR 0001 §1, §2, §4, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  pen- and node-tool state machines, selection, hit-testing and command
  dispatch are plain state and pure functions in `curvyo-ui-core`; the
  frontend renders state and forwards input events and holds no editing logic.
  The path is drawn in the WebGL2 canvas via `wgpu` from
  `curvyo-render-core`'s draw list, and §5 binds every live drag: per-frame
  data crosses as typed arrays, never as JSON of the document or the
  selection. **ADR 0001's owed WebKitGTK measurement comes due here** — see
  the prerequisite below.
- [ADR 0004 §1, §2, §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  a drawn path persists because it is in `document.loro` like everything else;
  no new container member and no bespoke serializer. §9's versioning rule is
  what forces the `format_version` bump below.
- [ADR 0006 §1](../../docs/adr/0006-license.md): the four new crate manifests
  and `frontend/package.json` carry `AGPL-3.0-or-later`.

## Deliberately not in scope for this slice

- **The `spike/booleans` decision is not due here.** ADR 0003 §3 ties it to
  "the first geometry story", which reads naturally as this one — but no
  acceptance criterion in this slice performs a boolean or an offset, and
  §3's spike judges degenerate-input robustness of a *polygon boolean*. Nothing
  here flattens for manufacturing. The spike is due with
  `boolean-operations` (slice 6); running it now would be work against an
  absent requirement (`CLAUDE.md` §5). Same for §4 (offsetting), §5
  (V-carving) and §6 (simplification): no code.
- **Shape-preserving node deletion needs curve fitting and is out of scope by
  the specification's own choice.** AC 13's "joined by one segment computed
  from their own existing handles" is exactly the behaviour that needs no
  `kurbo` fitting pass. Confirmed as a correct scope line, not an oversight:
  Inkscape's refitting delete is an ADR 0003 §6 simplification problem.
- **Collaboration ships nothing here.** No relay socket, no awareness
  transport, no `curvyo-crypto-core`, no `keyring.log` (ADR 0008 §7: local
  `.curvyo` is plaintext at rest by decision). The *data model* underneath is
  collaboration-safe as of this slice — that is the point of the anchor schema
  below — but the only channel is a local file.
- **Undo ships nothing here** (`undo-redo`, slice 5). Every operation in this
  slice writes a commit that slice 5's peer-scoped undo (ADR 0009 §1) will be
  able to revert without changes to this slice's commands, because ADR 0002 §9
  commands carry no inverse. AC 4's Escape is **not** an undo and must not be
  built as one.
- **`curvyo-vectorize-core`, `curvyo-library-core`, `curvyo-plugin`,
  `curvyo-sync-server`** get no code. ADR 0011 §3's `ui-core → vectorize-core`
  edge is not exercised; declare it when `raster-trace` (slice 9) needs it.

## Prerequisite this slice inherits

- **ADR 0001's WebKitGTK canvas measurement, owed since slice 1 and now due.**
  `specs/0001-project-file-foundation/adrs.md` deferred it here explicitly, and
  ADR 0001's consequences call it "the one failure that would invalidate this
  ADR rather than cost a refactor". It is a throwaway spike on
  `spike/webkitgtk-canvas`, never merged, run **before** the canvas work in
  `plan.md`, with the result recorded as a dated note in this file. What counts
  as measured: a synthetic scene at the order of magnitude ADR 0001's context
  names (thousands of path nodes — use 50 000 to leave headroom), on Linux
  WebKitGTK with `wgpu` → WebGL2, holding interactive frame rate under
  sustained pan/zoom, plus a live single-node drag whose input-to-pixel
  latency stays within one frame. A failure is not a performance ticket; it is
  an ADR superseding 0001 §4.

- **2026-10-03: measured. PASS — ADR 0001 §4 stands, no superseding ADR.**
  `wgpu` → WebGL2 in a real WebKitGTK 2.52.6 webview via `wry`/`tao` (the
  webview library Tauri uses), Arch Linux/X11, NVIDIA Quadro P1000 with the
  proprietary driver 580.178.04. **50 000 nodes held a vsync-locked ~60 fps
  for 600 frames** under continuous pan/zoom with a per-frame partial
  instance-buffer write standing in for a live single-node drag (60 frames per
  ~960 ms, no dropped interval). The prerequisite is discharged. Two
  implementation requirements fall out of it, neither of them an architecture
  decision:

  1. **The Linux build must disable WebKitGTK's DMA-BUF renderer before the
     webview starts** (`WEBKIT_DISABLE_DMABUF_RENDERER=1`). Measured on this
     machine: with it at its default, a WebGL2 context is acquired and
     reported as `WebGL 2.0` but **zero frames ever render** — for `wgpu` and
     for hand-written WebGL2 alike. This is the known WebKitGTK/NVIDIA
     DMA-BUF problem, not ours. The Tauri host sets it; it must be a startup
     requirement with a test, because the failure mode is a silent blank
     canvas, not an error.
  2. **The canvas layer reconfigures the `wgpu` surface on every resize** —
     set `canvas.width`/`canvas.height`, call `surface.configure()` with the
     new size, then render, all in one frame. Standard `wgpu` usage, verified
     here over 44 size changes. It is required for *correctness* (a stale
     surface composites at the wrong size); it is **not** a crash fix — see
     below.

  **The segfault reported from the first run of this spike is not caused by
  `wgpu`, by surface resizing, or by anything this slice controls.** It
  reproduces with **no resize of any kind** and, identically, from a plain
  hand-written WebGL2 page with no `wgpu`, no wasm and no Rust in the page at
  all — same `libnvidia-eglcore` stack, frame for frame. A page with no GL
  context does not reproduce it. It happens on **webview teardown**, after
  rendering has completed successfully, in the web process that is already
  exiting. Recorded in `docs/technical-debt.md`; it blocks nothing here.

## Feature-local decisions

- **2026-10-03: the anchor schema, and the three merge choices inside it.**
  This is the slice's one piece of protected ground (the document model), so
  it is written out in full and kept to exactly what the 14 criteria need:

  ```text
  path  = tree node (ADR 0002 §5), fields:
            closed        : bool            LWW register        (AC 5)
            stroke_width  : Length          LWW register        (AC 6)
            stroke        : Color           LWW register        (AC 6)
            fill          : None            LWW register        (AC 6)
            anchors       : movable list of anchor (ADR 0009 §3)
  anchor = map, fields:
            id            : AnchorId        written once        (AC 7–14)
            point         : Point           ONE LWW register    (AC 8, 10)
            handle_in     : Vec2 (relative) ONE LWW register    (AC 9, 14)
            handle_out    : Vec2 (relative) ONE LWW register    (AC 9, 14)
            kind          : Corner | Smooth LWW register         (AC 11)
  ```

  Three of those lines are decisions rather than transcription:

  1. **A point is one register, not two scalars.** `point`, `handle_in` and
     `handle_out` each merge as a single composite value. Splitting them into
     `x`/`y` fields would let two peers dragging one handle produce a vector
     with A's `x` and B's `y` — a third value neither peer asked for, which is
     precisely what ADR 0009 rejected as option D. ADR 0009 §3's phrase
     "transform components … are per-field LWW registers" must not be read
     down to per-coordinate here.
  2. **Handles are stored relative to their own anchor, not as absolute
     document points.** This is what makes AC 8 ("the node moves together with
     both of its handles, unchanged relative to the node") a single write of
     `point` instead of three writes that must agree, makes AC 9's mirror a
     negation of one `Vec2`, and makes AC 10 one write per selected anchor.
     Under concurrency it is also the only version that holds up: one peer
     moving the node and another dragging its handle write disjoint fields and
     both survive, where absolute handles would leave the handle stranded in
     document space. **A retracted handle is the exact zero vector**, so "this
     segment is a line" (AC 14) is a derived property of a value we write, not
     a classification needing a `Tolerance`, and no `is_line` flag exists.
  3. **`kind` is stored, not derived from geometry.** AC 11's smooth → corner
     conversion "leaves its two handles exactly where they are but they no
     longer move together", so a corner node whose handles happen to be
     mirrored must still behave as a corner. Geometry cannot tell them apart;
     the field must exist.

  `AnchorId` is minted by the creating peer and is globally unique, following
  ADR 0002 §5's rule for `NodeId` rather than relying on a Loro movable-list
  element handle: it is what ephemeral selection (ADR 0009 §2), AC 10's
  multi-selection and AC 12's neighbour updates refer to, and it must stay
  valid across a concurrent insert or delete elsewhere in the list. Like the
  peer id (slice 1, amended 2026-10-03), it is **passed into**
  `curvyo-document-core`, never minted there — a `*-core` crate reaches no
  entropy source (`CLAUDE.md` §6, ADR 0011 §6).

- **2026-10-03: the path/node crate boundary, and the rule that sets the
  precedent for every later editing slice.** The dividing question is *does
  this code need to know what a cubic Bézier is?*

  - `curvyo-document-core` — the schema above, the `Command` variants that
    write it, and **elementary arithmetic on its own newtypes**: add, subtract,
    scale, negate, normalize, length on `Point`/`Vec2`. On that basis AC 1, 2,
    8, 9, 10, 11, 13 and 14 are document-model bookkeeping and need no kernel.
    Mirroring a smooth node's handle is `handle_in = -handle_out`; AC 11's
    corner → smooth tangent is the normalized chord between the two
    neighbouring anchors scaled to a default length; AC 14's "make curve" sets
    the two adjoining handles to a default fraction of the chord. None of that
    evaluates a curve. ADR 0003 §1's "the only place geometric algorithms
    live" is not read as forbidding `impl Add for Vec2` in the crate that
    owns `Vec2`.
  - `curvyo-geometry-core` — anything that does need the curve: flattening a
    segment for hit-testing, nearest-point-on-segment, and de Casteljau
    subdivision for AC 12. `kurbo` per ADR 0003 §2.
  - **Commands carry resolved geometry, never geometric intent.** ADR 0011 §3
    is explicit that `document-core` cannot reach the kernel and that
    `curvyo-ui-core` executes commands. So AC 12 is: `ui-core` hit-tests the
    click and asks `geometry-core` for the parameter and the subdivision, then
    dispatches `InsertAnchor { after, anchor, prev_out, next_in }` carrying
    numbers. An `InsertAnchorAt { segment, t }` command would force
    `document-core` to subdivide and invert ADR 0011 §3's edge. Every later
    slice follows this shape — slice 6's boolean result is a path computed in
    `geometry-core` and handed to `document-core` as data.
  - `curvyo-ui-core` — pen and node tool state machines, the in-progress
    path, selection of nodes/handles/segments, hit-testing with an explicit
    `Tolerance`, command dispatch. A **segment has no document identity**:
    AC 14's selected segment is the local pair of adjacent `AnchorId`s, which
    is why "segment selected" needs no field and no merge rule.
  - `curvyo-render-core` — the path's draw list, and the node/handle/segment
    decorations. ADR 0011 §3 gives it `→ document-core` only, so it **cannot
    read selection from `ui-core`**: its entry point takes (version snapshot,
    view transform, a decoration input of `AnchorId`s and flags built from
    `document-core` types), and `curvyo-editor-wasm` passes `ui-core`'s
    selection across as binding, not logic (ADR 0001 §3). Decorations are
    drawn in the GPU draw list rather than as DOM overlays, so pan/zoom has
    one coordinate system; ADR 0001 §4's DOM-overlay allowance stays reserved
    for the accessibility story, which is the `ux-engineer`'s to shape.
  - `curvyo-app` / `frontend/` — tool palette, the node-tool action surface
    for AC 11 and AC 14, keyboard routing (Escape, Delete), and nothing else.

- **2026-10-03: a pen session is one commit, and that is what makes AC 4
  buildable at all.** From the first click to the double-click, close-path or
  Escape, the in-progress path lives **only** in `curvyo-ui-core` as
  ephemeral state (ADR 0009 §2). The document gains exactly one commit, at
  AC 3's finish or AC 5's close; AC 4's Escape drops local state and writes
  nothing. Writing each clicked node into the document as it is placed would
  make AC 4 require undo — which this slice explicitly does not have — and
  would break ADR 0002 §9's one-commit-per-interaction. Each post-finish
  interaction is likewise one commit: one node drag, one handle drag, one
  multi-node drag, one convert, one insert, one delete, one line/curve switch.

- **2026-10-03: `format_version` goes to 2.** ADR 0004 §9 has writers always
  write current and readers refuse a newer file outright. A slice-1 reader
  opening a slice-2 file would accept `format_version: 1` and silently ignore
  every path node, which is the silent geometry loss §9 exists to prevent. The
  bump lives in `manifest.json` (slice 1's feature-local decision) and the
  `document.json` view widens to carry paths. Migration from version 1 is
  empty by construction — a version-1 document has no path nodes — and that
  sentence is the written migration policy for this step.

- **2026-10-03: `ViewTransform` is pan + uniform zoom, not a general affine
  matrix.** ADR 0011 §3 names "the affine transform type from
  `document-core`, passed to both [`render-core` and `ui-core`]" without
  pinning its shape. This slice's canvas never rotates or skews, and no path
  node carries its own transform yet (ADR 0002 §5's per-node affine transform
  is not implemented here — this slice's anchor schema above has no
  `transform` field), so a 6-component matrix would be unexercised generality
  (`CLAUDE.md` §5 YAGNI). `curvyo-document-core::ViewTransform` is a `scale`
  (screen pixels per document mm) and an `origin` (`Point`), with
  `document_to_screen`/`screen_to_document`. `render-core` uses it for
  acceptance criterion 6/7's screen-space-constant decoration sizing; the
  host (not `curvyo-ui-core` itself) uses it to turn a raw pointer event
  into the document `Point` the tools in this slice already take. A later
  slice that needs real per-node rotation is free to generalize this type or
  add a separate one — this name and shape are not a commitment past this
  slice's own needs.

## Flagged to the lead

1. **The specification's prose cites ADR 0002 §5 for *anchor* identity**
   ("every node is a CRDT-tracked entity with a stable identity (ADR 0002
   §5)"). §5 mints `NodeId`s for *tree* nodes; anchor identity comes from
   ADR 0009 §3's movable list, and this file mints `AnchorId` on §5's pattern
   rather than under its authority. The substance the specification asserts is
   correct and no acceptance criterion depends on a node index, so this is a
   pointer to correct for readers, not a conflict and not a change the
   product-owner needs to make. **No AC conflicts with any ADR.**
2. **No new crate is needed.** Everything fits the twelve of ADR 0011 §1, and
   this slice does not trip §8's `curvyo-model-core` trigger either — though
   the slice-1 review note (Loro needs a JS host on `wasm32`) still stands and
   still points at that extraction.
3. **One observation for the plugin-host story, no action now.** ADR 0011 §3
   gives `curvyo-plugin → document-core` and no geometry edge, so under the
   "commands carry resolved geometry" rule above a plugin cannot author an
   AC 12-style split: it has no kernel to compute it with. That is a real gap
   in ADR 0005's model, and it belongs to the plugin-host story (ADR 0011 §8),
   not here.
4. **ADR 0001's WebKitGTK measurement is a prerequisite task in `plan.md`,
   ahead of the canvas work**, and a failure is an ADR, not a ticket. It is the
   one thing in this slice that can invalidate an accepted decision.

## Architect review notes (2026-10-03, PR #7)

- **The `Document` methods in `paths.rs` are the command funnel for this
  slice; a serializable `Command` enum is deferred to `undo-redo` (slice 5).**
  ADR 0002 §9's enum has no consumer before the history view and the plugin
  host, and turning one funnel module into an enum later is internal. What is
  not deferred: **each mutating method ends in exactly one Loro commit** with a
  human-readable label. Without it every edit since the last save lands in one
  auto-commit transaction, which slice 5's undo cannot split. A method that
  can refuse resolves every id before its first write, so a refusal writes
  nothing.
- **`fill` is not stored while it is always `None`.** An absent key reads as
  `None`, so slice 4 adds the register without a format change.
- **`flatten_segment` leaves `curvyo-geometry-core`.** Hit-testing landed on
  nearest-point-on-segment, so flattening had no production caller (`CLAUDE.md`
  §5). The crate's scope for this slice is nearest-point and subdivision.
- **`document.json` writes ids as strings.** `AnchorId` (128 bit) and the
  `NodeId` peer (63 bit) exceed the 2^53 integers JSON readers such as
  JavaScript and `jq` keep exactly, and ADR 0004 names scripted reading and
  recovery as this file's purpose. `AnchorId` uses the same 32-digit hex form
  as the Loro value. `kind` is lowercase to match.
- **Opening a `.curvyo` validates the path tree before it returns a `Document`.**
  A container whose path data does not match the schema above is refused with
  `OpenError::Damaged` (project-file-foundation AC 7: a named error, not a
  crash). After that check, the read helpers' `// invariant:` comments hold
  for every reachable document.
- **2026-10-03: a press and release with no pointer movement writes nothing.**
  This applies to node drags (AC 8, AC 10) and to handle drags (AC 9). If the
  pointer-up position is identical to the pointer-down position,
  `pointer_up` returns `NoOp` and does not call the document. Neither AC 8 nor
  AC 9 says anything about a drag of zero length. They describe a drag "to a
  new position", so this is an edge case the specification leaves open. It
  is settled here, not by the test, for three reasons:
  1. **Under ADR 0009 §3, writing the same value again still changes the
     document.** `point` and both handles are LWW registers. A rewrite of an
     unchanged value is a new operation with a newer clock. It can win
     against a collaborator's concurrent real drag of the same node and undo
     their move on merge. A click must not be able to do that.
  2. **ADR 0002 §9 counts interactions that change the document.** Selecting
     a node is ephemeral state (ADR 0009 §2), and so is clicking an
     already-selected one. A commit for each such click would put an empty
     step into slice 5's peer-scoped undo for every selection click. The
     maker would then press undo and see nothing happen.
  3. **The handle path makes it visible today.** `Drag::Handle` writes the
     absolute pointer position. A click within hit tolerance of a handle tip
     but not exactly on it therefore moves the handle by up to the tolerance.
     That is a geometry change from a click. The handle drag records where
     the pointer went down, measures the movement from there in the same way
     node drags do, and writes nothing when the pointer did not move.

  The check compares the two pointer positions for identity. It is not a
  geometric `Tolerance` (`CLAUDE.md` §5). The host turns the same pixel into
  the same `Point`, so "did not move" is exact. A screen-pixel drag threshold
  against hand jitter is a separate interaction decision for the
  `ux-engineer`. It is not part of this fix.
