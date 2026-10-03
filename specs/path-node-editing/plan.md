# Plan for "Path and node editing: draw and edit a Bézier path with a pen tool"

Canvas-perf spike (ADR 0001 §4 prerequisite): **PASS**, recorded
2026-10-03 in `specs/path-node-editing/adrs.md` and `docs/technical-debt.md`.
Two production requirements fall out of it and are tasks below (6, 10):
`WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux startup, and reconfiguring the
`wgpu` surface on every resize.

## Affected crates/modules

- `vecmanf-document-core` — widen the document model: `Path` tree nodes,
  `Anchor` movable-list entries, `NodeId`/`AnchorId`, the path `Command`s,
  `format_version` → 2, `document.json` view widened.
- `vecmanf-geometry-core` (**new crate**, authorized by ADR 0003 §1 and named
  in ADR 0011 §1; stood up here per `adrs.md`) — `kurbo`-backed
  flatten-for-hit-test, nearest-point-on-segment, subdivide-at-parameter.
  Nothing else.
- `vecmanf-ui-core` (**new crate**, ADR 0001 §1, ADR 0011 §1) — pen/node tool
  state machines, hit-testing, selection, command dispatch. The in-progress
  pen path is ephemeral state here only (ADR 0009 §2) — ADR 0002 §9's
  "a pen session is one commit" lives in this crate, not in document-core.
- `vecmanf-render-core` (**new crate**, ADR 0001 §4, ADR 0011 §1) — document
  snapshot + view transform + decoration input → flat draw list, tessellated
  with `lyon`.
- `vecmanf-editor-wasm` (**new crate**, ADR 0001 §3, ADR 0011 §1) — thin
  `wasm-bindgen` facade: binds the above three crates, owns the `wgpu`
  device/surface and GPU submission. No logic of its own.
- `vecmanf-app` — Linux startup sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`
  before the webview is built (task 10).
- `frontend/` — tool rail (Pen/Node), canvas rewritten to host the wasm
  module's `<canvas>`, keyboard shortcuts (`B`/`N`/Enter/Escape/Delete), the
  node-tool contextual actions bar and context menu.

No crate outside this list changes. No new crate beyond the four ADR 0011 §1
already named.

## Tasks

### Core data model (document-core), test-first

- [x] 1. `NodeId` (wraps a Loro `TreeID` privately, ADR 0002 §5) and
      `AnchorId` (opaque `u128` from a caller-supplied `(peer, counter)`
      pair — minted by `vecmanf-ui-core`, never inside this crate, per
      `adrs.md`). `Point`, `Vec2` newtypes (ADR 0002 §3) with the
      elementary arithmetic `adrs.md`'s crate-boundary decision assigns here
      (add/sub/scale/negate/normalize/length). (infra for all ACs)
- [x] 2. `AnchorKind` (`Corner | Smooth`), stored not derived (`adrs.md`
      decision 3). `Path` tree node schema: `closed`, `stroke_width`,
      `stroke`, `fill`, `anchors` movable list of anchor maps with
      `id`/`point`/`handle_in`/`handle_out`/`kind`, `point`/`handle_in`/
      `handle_out` each ONE LWW register (`adrs.md` decisions 1–2: a
      `LoroValue::List([x,y])`, never two scalar fields). (AC 1, 2, 6)
- [x] 3. `Document::create_path(anchors, closed) -> NodeId` — the pen
      session's single commit (AC 1, 2, 3, 5).
- [x] 4. Read model: `Document::path_ids()`, `Document::path(NodeId) ->
      Option<PathSnapshot>` (plain, serializable — the ADR 0002 §5 "derived
      local read model" permission), used by render-core and ui-core.
      (AC 6, 7)
- [x] 5. `Document::move_anchors(path, &[(AnchorId, Point)])` — one commit
      for a whole drag, handles untouched because they're anchor-relative
      (AC 8, 10).
- [x] 6. `Document::set_handle(path, anchor, slot, value)` — mirrors the
      opposite handle itself when `kind == Smooth` (`handle_in =
      -handle_out`), touches only the dragged handle when `Corner` (AC 9).
- [x] 7. `Document::convert_anchor_kind(path, anchor, kind)` — corner→smooth
      computes the tangent from the neighbour anchors already in the path
      (chord, normalized, scaled to a default length — elementary vector
      math, no kernel); smooth→corner only flips `kind`, handles untouched
      (AC 11).
- [x] 8. `Document::insert_anchor(path, after, new_anchor, prev_out,
      next_in)` — pure bookkeeping over caller-resolved geometry (AC 12).
- [x] 9. `Document::delete_anchors(path, &[AnchorId])` — removes the path
      object outright when fewer than two anchors would remain (AC 13).
- [x] 10. `Document::set_segment_line`/`set_segment_curve(path, start, end)`
      — retract/extend the two adjoining handles by a default chord
      fraction; no kernel involved (AC 14).
- [x] 11. `format_version` → 2 (`CURRENT_FORMAT_VERSION`), `document.json`
      view widened to carry paths; migration from version 1 is empty by
      construction. New golden fixtures in `tests/fixtures/` for a
      multi-path, multi-handle-type document; existing fixtures regenerated.
      (`adrs.md`, "format_version goes to 2")

### Geometry kernel (new `vecmanf-geometry-core`), test-first

- [x] 12. `flatten_segment(start, handle_out, handle_in, end, tolerance) ->
      Vec<Point>` via `kurbo::CubicBez` — used for segment hit-testing
      (AC 12, 14) and rendering's stroke tessellation input.
- [x] 13. `nearest_point_on_segment(..., query, tolerance) -> (t, distance,
      point)` — AC 12's "double-click a point on a segment" and AC 14's
      "click on a point of a segment".
- [x] 14. `subdivide_at_parameter(..., t) -> (left_handle_out, new_point,
      new_handle_in, new_handle_out, right_handle_in)` via de Casteljau —
      AC 12's split, carrying resolved geometry back to `ui-core` (`adrs.md`
      "commands carry resolved geometry, never geometric intent").
- [x] 15. `cargo build --target wasm32-unknown-unknown` clean, no `unsafe`.

### Interaction layer (new `vecmanf-ui-core`), test-first

- [x] 16. `PenTool` state machine: idle → placing (node-by-node) → finished/
      discarded. Ephemeral in-progress path (points + handles + per-node
      corner/smooth), never touches `document-core` until finish/close
      (AC 1–5; `adrs.md` "a pen session is one commit").
- [x] 17. Pen tool: plain click → corner node + line segment (AC 1); click-
      drag → symmetric handles + curve segment (AC 2); finish action
      (double-click/Enter) → one `create_path` commit, open (AC 3); Escape →
      drop ephemeral state, zero commits (AC 4); click on the in-progress
      path's own first node (≥3 nodes) → closes and commits (AC 5).
- [x] 18. `NodeTool` state: selection (nodes/handles/segments, click +
      shift-click, AC 7, 10, 14), hit-testing against a `PathSnapshot` with
      an explicit `Tolerance` (8px node/handle, 4px segment, per
      `docs/design-system.md`), drag-in-progress geometry (ephemeral, ADR
      0009 §2) committed as one `move_anchors`/`set_handle` call on release
      (AC 8, 9, 10).
- [x] 19. Node-tool actions → one command each: convert corner/smooth
      (AC 11), insert on double-click (hit-tests via geometry-core, then
      `insert_anchor`, AC 12), delete selected (AC 13), make line/make curve
      on a selected segment (AC 14).
- [x] 20. `cargo build --target wasm32-unknown-unknown` clean.

### Rendering (new `vecmanf-render-core`), test-first where the data shape allows it

- [x] 21. Draw-list builder: path snapshot + view transform → flattened
      stroke geometry (0.25 mm, black, no fill — AC 6) via `lyon`, plus
      node/handle/segment decoration primitives from a `DecorationInput`
      (selected/hovered ids and flags) built by the wasm facade from
      `ui-core`'s selection — this crate never reads `ui-core` directly
      (`adrs.md` crate-boundary decision).
- [x] 22. Screen-space-constant sizing for every decoration (7px node, 6px
      handle, 1px handle line, +2px segment overlay, per
      `docs/design-system.md`), computed from the view transform, not
      baked into document units.
- [x] 23. `cargo build --target wasm32-unknown-unknown` clean.

### wasm facade (new `vecmanf-editor-wasm`)

- [x] 24. `wasm-bindgen` bindings: open/create document, dispatch pointer/
      keyboard input to `ui-core`, pull the draw list each frame, own the
      `wgpu` device/surface and submit — no editing logic of its own
      (ADR 0001 §3).
- [x] 25. Surface reconfigure on every resize: update canvas size, call
      `surface.configure()`, then render, all before presenting the next
      frame (`adrs.md`'s PASS note, requirement 2 — correctness, not the
      measured crash, which is unrelated upstream teardown noise).
- [x] 26. `cargo build --target wasm32-unknown-unknown` clean.

### Frontend and host wiring

- [x] 27. Tool rail: 48px left-docked vertical rail, Pen/Node buttons,
      `aria-label`s, tooltips, active-tool styling, `B`/`N` canvas-focus
      shortcuts, Pen default on an empty canvas.
- [x] 28. Canvas component hosts the wasm module's `<canvas>`, forwards
      pointer/keyboard events, renders every frame from the draw list.
      Cursors per `specification.md`'s UX notes (pen-nib, plain arrow for
      node tool). Known gap, flagged to the lead: the close-path cursor
      variant (pen-with-small-circle when hovering the in-progress path's
      own first node) is not implemented — the hover ring signal on that
      node still shows, so the close target isn't signalled by zero cues,
      just one instead of two.
- [x] 29. Contextual tool-controls bar (node tool only): Insert, Delete,
      Make corner, Make smooth, Make line, Make curve, disabled when
      inapplicable; same actions on a right-click context menu; Delete/
      Backspace keys bound; Escape clears selection (not a pen discard).
- [x] 30. `vecmanf-app`: `WEBKIT_DISABLE_DMABUF_RENDERER=1` set on Linux
      before `tauri::Builder` runs, with a unit test that the setup
      function actually sets it (`adrs.md`'s PASS note, requirement 1 — the
      failure mode is a silent blank canvas, not an error, so this needs a
      test rather than a hope). Uses `unsafe { std::env::set_var(...) }`
      (stable Rust now requires `unsafe` for this call) — flagged to the
      lead per `CLAUDE.md` §5's "unsafe anywhere else needs an ADR".

### Container format

- [x] 31. Golden fixtures regenerated for `format_version = 2` with paths
      of both anchor kinds, open and closed, in `tests/fixtures/`. A
      fixture at the old `format_version = 1` still opens (empty path list,
      the written migration policy). (Done alongside task 11:
      `tests/fixtures/paths_v2.vmf` and `tests/fixtures/format_version_1.vmf`,
      pinned by `vecmanf-document-core/tests/container_fixtures.rs`.)

## Validation

- Unit tests in `vecmanf-document-core` for every `Document` method above,
  including the three merge-relevant behaviours `adrs.md` calls out (point
  as one register, handle mirroring, `kind` surviving a geometry-alike
  state).
- Unit tests in `vecmanf-geometry-core` against known closed-form Bézier
  points (e.g. a quarter-circle approximation) for flatten/nearest/
  subdivide.
- Unit tests in `vecmanf-ui-core` driving each tool's state machine through
  every acceptance criterion's Given/When/Then as a sequence of input
  events, asserting the resulting ephemeral state and the exact commands
  dispatched (no document-core or geometry-core mocking needed — real
  instances, since both are pure and fast).
- Golden-file tests for the widened `.vmf` container in
  `vecmanf-document-core/tests/`.
- `cargo build --target wasm32-unknown-unknown` for all four `*-core`
  crates (gate requirement, also tasks 15/20/23/26 above).
- Manual check in the Tauri dev build: draw an open and a closed path,
  select/move/convert/delete nodes, insert a node, switch a segment
  line↔curve, confirm the 0.25 mm stroke and no fill, confirm Escape
  discards and the tool rail/shortcuts match `docs/design-system.md`.
- Full `CLAUDE.md` §7 gate before reporting done.
