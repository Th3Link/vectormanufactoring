# ADRs for "Canvas navigation and a general Select tool"

This slice adds no geometry kind and no stored field. It is the first slice
whose view changes at runtime, and the first tool that acts on every object
kind at once. Its protected ground is small: moving and deleting objects
writes to the document. Both are settled below, and both reuse registers
that already exist. **No new crate, no new external dependency, no
`format_version` bump, no ADR amendment.**

All 24 acceptance criteria can be built against the ADRs as they read today.
None conflicts with an ADR or with a decision of slices 1–3. Checking the
code turned up one shipped gap (AC 11) and two rendering limits at the zoom
extremes (AC 7). They are under "Flagged to the lead".

## Depends on

- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): pan,
  zoom, canvas size, the pan gesture in flight, the Select tool's selection,
  hover and in-flight move offset are **ephemeral**. None of them is an
  operation, none is written to `.vmf`, and none is undoable. A New or Open
  starts with the default view. Selection holds `NodeId`s and resolves them
  lazily, dropping ids that no longer exist.
- [ADR 0002 §2, §3, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  a move is a `Vec2` in mm, and every hit test takes an explicit `Tolerance`
  derived from screen pixels at the current zoom (already done in `Session`).
  **One interaction is one commit**: one Select-tool drag of N objects is one
  commit, one Delete of N objects is one commit (AC 18–21), and a press and
  release with no movement writes nothing (slice 2's rule).
- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  paths and primitives are nodes in one tree with one z-order, so "any
  object" is one id space (`NodeId`) and one selection. §5's per-node affine
  transform is **still not implemented**. See "a move rewrites geometry"
  below for why this slice does not need it.
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md): the
  registers a move writes already exist and already have merge rules: anchor
  `point` (slice 2) and `rect_bounds` / `ellipse_frame` / `star_frame`
  (slice 3). A delete is a tree delete.
- [ADR 0001 §1, §3, §4, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  navigation and the Select tool are plain state in `vecmanf-ui-core`. The
  facade forwards input and holds no logic. The selection box is drawn in
  the WebGL draw list, not the DOM. Pointer and wheel input crosses as
  scalars and the view as one uniform, never as JSON.
- [ADR 0003 §1, §2, §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  the tight bounds of a cubic segment evaluate the curve (extrema), so they
  belong in `vecmanf-geometry-core` via `kurbo`. Display tessellation stays in
  `render-core` at its own tolerance. This slice changes that tolerance from
  fixed mm to screen pixels.
- [ADR 0011 §2, §3](../../docs/adr/0011-workspace-and-crate-layout.md):
  every piece fits an existing crate over an existing edge (see "the crate
  boundary" below).
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  nothing in the stored format changes, so `format_version` stays 3.
- [`specs/0002-path-node-editing/adrs.md`](../0002-path-node-editing/adrs.md):
  the `ViewTransform` decision, "commands carry resolved geometry", "a press
  and release with no pointer movement writes nothing", and the spike's PASS
  note, requirement 2 (reconfigure and render in the same frame on resize).
- [`specs/0003-primitive-shapes/adrs.md`](../0003-primitive-shapes/adrs.md):
  the shared object selection, the "tool mismatch" rule, handle layout in
  `ui-core` passed through `DecorationInput`, and "object to path keeps the
  `NodeId`".

## Deliberately not in scope for this slice

- **The per-node transform of ADR 0002 §5.** It becomes due with the first
  story that rotates or scales an object as a whole (the Select tool's
  out-of-scope transform handles, or `layers-and-grouping`'s group
  transform). Adding it later is additive: a missing key reads as identity,
  so no migration is needed.
- **Filled-interior hit-testing.** No object has a fill yet.
  `stroke-and-fill-styling` (slice 5) must extend this slice's single
  `hit_test_object` so that a click inside a filled closed object selects
  it. Wide strokes (distance to the outline minus half the stroke width)
  need the same change. Both belong in that one function, not in a second
  one.
- **Saving the view in the file** (Inkscape's `namedview` zoom/centre). The
  view is ephemeral by ADR 0009 §2. If a story asks for it, it is per-peer
  data and must stay outside the shared document.
- **devicePixelRatio.** The canvas backing store equals its CSS size, so
  rendering is upscaled on HiDPI displays. This is not new, and zoom does
  not make it worse. Recorded in `docs/technical-debt.md`.

## Feature-local decisions

- **2026-10-05: navigation state lives in a `viewport` module in
  `vecmanf-ui-core`. `ViewTransform` keeps its shape.** The existing type
  (`document-core/src/units.rs`: `scale` px/mm plus `origin`, the document
  point at screen (0, 0)) is enough for everything here. Nothing in it
  needs to change:
  - *Zoom toward the cursor (AC 6)* is a recomputed origin, not an in-place
    scale: for cursor pixel `s`, `p = screen_to_document(s)`, then
    `origin' = p − s / scale'`. The point under the cursor is fixed exactly,
    including when the new scale was clamped.
  - *Pan (AC 1–4)* is `origin' = origin − Δpx / scale`. Middle-drag and
    Space+drag store the document point under the cursor at press time and
    keep it under the cursor (AC 3), so the drag does not accumulate
    rounding error.
  - *Range (AC 7, 8)*: the module stores a validated `Zoom` newtype (a
    dimensionless factor, `0.02 ..= 80.0`, clamped; same pattern as
    `InnerRatio`) and derives `scale = zoom × PX_PER_MM_AT_100`. The readout
    (AC 9) is `zoom × 100`, so the limits read exactly 2 % and 8000 % with
    no floating-point residue. `PX_PER_MM_AT_100 = 96 / 25.4` moves out of
    `frontend/` (`CSS_PX_PER_MM`) into this module, because the limits are
    defined in terms of it.
  - *Precision*: everything is `f64` up to the GPU uniform. Scale spans
    0.076–302 px/mm. No overflow is reachable, and the only real precision
    risk is the f32 cast, handled below.

  The module holds the `ViewTransform`, the canvas size in CSS px and the pan
  gesture in flight. Its one job is "which part of the document is on
  screen". `Session` owns one instead of a bare `ViewTransform`.
  `render-core` still receives the plain `ViewTransform` (it cannot see
  `ui-core`). Rejected: putting zoom limits and gesture state on
  `ViewTransform` in `document-core`. That is view policy, and
  `document-core` is the document model.

- **2026-10-05: all screen↔document conversion happens in Rust.** Today
  `frontend/src/hooks/useEditorSession.ts` divides by a constant to get
  document mm, and `Canvas.tsx` multiplies by it to place the live readout.
  With a live view, a second copy of that conversion is a guaranteed
  mismatch. The wasm pointer methods (`pointer_down`, `pointer_hover`,
  `pointer_up`, the new `double_click`) take **canvas-relative CSS pixels**,
  and `Session` converts them through its viewport. Anything the DOM
  positions (the live readout, the status-bar cursor position, later the
  mini-toolbar anchor) is returned already converted. `set_view` leaves the
  wasm API, the double-click distance check runs in pixels, and the
  frontend no longer reads `CSS_PX_PER_MM`. This replaces slice 2's "the
  host converts" wording. The tools in `ui-core` still take document
  `Point`s, unchanged.

- **2026-10-05: navigation input is offered to the viewport before the
  active tool.** A wheel event, a middle-button press, and a primary press
  while Space is held go to the viewport and are never forwarded to the
  tool. The tool's in-progress state is therefore untouched by construction
  (AC 5), in every tool (AC 24), with no per-tool code. Hover still updates
  during a pan. During a middle-drag the document point under the cursor
  does not change, so this cannot disturb a rubber band or a live preview.
  Frontend requirements, which are input translation rather than logic:
  register the wheel listener with `{ passive: false }` (React's `onWheel`
  is passive and cannot `preventDefault`, so Ctrl+wheel would zoom the
  webview page itself); normalize `deltaMode` to pixels; apply both
  `deltaX` and `deltaY` so a two-finger trackpad scroll pans diagonally;
  capture the pointer for middle and Space drags; `preventDefault` middle
  `pointerdown` (Windows autoscroll, X11 primary paste) and Space `keydown`
  (it would activate a focused button).

- **2026-10-05: resize keeps the centre (AC 10) and renders in the same
  callback (AC 11).** `viewport.resize(w, h)` keeps the zoom and moves the
  origin by `((w₀ − w₁)/2, (h₀ − h₁)/2) / scale`, so the document point at
  the viewport centre stays there. This is Inkscape's default behaviour:
  with "sticky zoom" off, its canvas keeps the visible-area midpoint and the
  zoom across a size change. Collapsing the right Properties panel
  (`docs/design-system.md`, 2026-10-05) is a resize like any other and gets
  the same behaviour. **AC 11 is not met by shipped code.** The
  `ResizeObserver` in `useEditorSession.ts` sets `canvas.width`/`height`
  (which clears the WebGL drawing buffer) and calls `resize`, but the next
  `render()` happens only in the following animation frame. Observers run
  after that frame's `requestAnimationFrame` callbacks and before paint, so
  one frame is composited empty. The fix is the third step of slice 2's
  requirement 2, which was never built: the observer callback calls
  `render()` synchronously after `resize()`. The wasm `resize` should do
  surface reconfigure, viewport resize and render as one call, so the host
  cannot get the order wrong.

- **2026-10-05: rendering at the zoom extremes.** Three requirements, all in
  `render-core` and `editor-wasm`, none of them a format change:
  1. **Display tolerance becomes screen-space.** `stroke.rs`'s
     `DISPLAY_TOLERANCE_MM = 0.05` is a 15 px chord error at 8000 %. Curves
     would show visible facets exactly where AC 7 promises precise node
     placement. Use `0.25 px / scale`. ADR 0003 §7's "separate, coarser than
     the kernel" still holds.
  2. **The GPU upload subtracts the view origin in `f64` before the f32
     cast.** `gpu.rs` converts every vertex each frame anyway. Today it
     casts absolute mm and adds a large f32 offset in the shader, which
     cancels catastrophically when panned far from the origin at high zoom.
     Pan is unbounded (the spec sets no limit), so this is reachable.
     Positions relative to the origin keep f32 error proportional to
     distance on screen, not distance in the document. The shader is
     unchanged; only the offset term changes.
  3. **A draw list depends on the view's scale, never on its origin.** This
     is already true (decoration sizes use `view.scale()` only), and it must
     stay true. It is what lets a later cache rebuild only on zoom, so that
     a pan rewrites one uniform (see `docs/technical-debt.md`).

- **2026-10-05: one object selection, shared by the Select tool and the
  shape tools.** `PrimitiveSelection` already holds plain `NodeId`s and
  knows nothing primitive-specific. It is renamed to `ObjectSelection`, and
  its doc comment says "any object". `Session` keeps the one instance it
  already has. A mixed path/primitive selection (AC 17–19) is a list of ids.
  Each id's kind comes from `Document::object(id)` at the point of use. No
  new type and no stored kind. The shape tools' "tool mismatch" rule
  already ignores ids of other kinds, so a Select-tool selection that
  contains paths changes nothing in the shape tools. "Object to path" stops
  removing the converted ids from the selection, because they are still the
  same objects (slice 3: the `NodeId` is kept). A multi-object conversion
  therefore stays selected together at object level, which covers half of
  the slice-3 debt entry. `NodeSelection` (anchors within one path) is
  unchanged and stays the node tool's own selection.

- **2026-10-05: one object hit test, no third copy.** Two loops already
  measure the distance from a point to a run of cubic segments:
  `hit_test.rs`'s segment test over path anchors and
  `shape_hit_test.rs`'s `hit_test_primitive` over `outline_of`. The Select
  tool needs the union of the two. Decided: a private helper computes the
  distance from a point to an anchor run (point, `handle_in`, `handle_out`,
  closed flag) via `nearest_point_on_segment`. A public
  `hit_test_object(&[ObjectSnapshot], Point, Tolerance) -> Option<NodeId>`
  uses it (paths through their anchors, primitives through `outline_of`).
  The nearest object within tolerance wins, and a tie goes to the topmost in
  z-order. `hit_test_primitive` becomes the same call restricted to the
  active shape tool's kind. The node tool's segment test may call the
  helper, but it must still return which segment was hit. Rejected: having
  the Select tool call `hit_test` and `hit_test_primitive` and compare
  results. That keeps two definitions of "near an outline", and slice 5's
  fill test would then have to go into both.

- **2026-10-05: one bounds rule, used by both drawing and the tools.**
  AC 14 extends the primitive selection box to paths. A path's box must be
  tight (a control-point hull floats visibly off a curve with long handles),
  and tight means curve extrema.
  - `vecmanf-geometry-core` gains `segment_bounds` (`kurbo`'s
    `ParamCurveExtrema::bounding_box`). It is a small, exact function.
  - The primitive box that `render-core/src/shape_preview.rs` computes
    privately (`bounding_box(shape)`, the frame box the shape tools' handles
    sit on) moves to `document-core` as plain arithmetic on its own types.
    `render-core` and `ui-core` both call it. AC 14 says "the same
    convention", so a star keeps its circumscribed-frame box.
  - `ui-core`'s `object_bounds(&ObjectSnapshot)` returns the frame box for a
    primitive and the union of `segment_bounds` for a path. The Select
    tool's boxes reach `render-core` as rectangles in the decoration input,
    the same way slice 3 passes handle positions. That gives one source for
    the box drawn now and the box that later transform handles, "fit
    selection" and the mini-toolbar anchor will use.

- **2026-10-05: a move rewrites geometry. There is no transform field.**
  `ObjectSnapshot::translated(Vec2)` in `document-core` is elementary
  arithmetic. It applies to a path's anchor `point`s (handles are relative,
  slice 2 decision 2, so they need no write) and to the origin or centre of
  a primitive's frame. `Document::translate_objects(&[NodeId], Vec2)` writes
  exactly those registers in **one commit**. It resolves every id before the
  first write and refuses stale ids with a typed error, like
  `convert_to_paths`. `ui-core` filters the selection against the current
  snapshot first. The live preview during the drag (AC 20 "live") renders
  `translated` snapshots and writes nothing (slice 3's `primitives_for_render`
  pattern). Preview and commit therefore share one implementation and agree
  exactly.
  Rejected: a per-node `transform` register now (ADR 0002 §5). It would
  merge better. One peer moving a path while another drags one of its
  anchors would keep both edits, where rewriting the anchors leaves the
  dragged anchor in old-frame coordinates under LWW. That is the same floor
  slice 2's multi-node drag (AC 10) already accepts. The cost of the
  transform field is that every reader would have to compose it: render,
  all five tools' hit tests, the node tool's drag inversion, shape handle
  layout, `outline_of` / object to path, and later SVG export and job
  generation. All of that would be built to exercise translation only, in a
  product with no live collaboration yet (`CLAUDE.md` §5). Baking also
  matches what Inkscape writes by default for a moved path. The field can
  still be added later without migration (see "not in scope").

- **2026-10-05: delete is a tree delete, one commit.**
  `Document::delete_objects(&[NodeId])` resolves every id first, refuses
  stale ids, deletes the tree nodes, and makes one labelled commit
  (AC 19, 21). It works the same for every kind because a delete never
  reads geometry. The anchors of a deleted path go with their node.
  Selections that still name them drop the ids on the next lazy resolve.

- **2026-10-05: double-click is one wasm entry point, routed in `Session`.**
  Today the TypeScript hook chooses per tool between `finish_pen`,
  `insert_at` and `pointer_up` when a double-click is detected. That is
  routing logic in the frontend, and a fourth case would make it worse.
  Decided: the host detects a double-click (unchanged, in pixels now) and
  calls `double_click(x, y)`. `Session` dispatches it: Pen finishes, Node
  inserts, Select hands off, and shape tools treat it as `pointer_up`, as
  today. The Select tool returns an outcome (the hit `NodeId` and its kind,
  or a miss). `Session` maps kind to `Tool` with **one** function,
  `tool_for(&ObjectSnapshot)`. The existing `shape_matches_active_tool` is
  then derived from it, so the two directions of the mapping cannot drift
  apart. For a primitive, the object selection already holds the id, so the
  shape tool shows its handles at once (AC 23). For a path (AC 22), the
  handoff switches to `Tool::Node` and clears `NodeSelection`. The node tool
  already draws every path's nodes, so the path is ready for node editing
  with no nodes selected. That is Inkscape's behaviour, and it needs no new
  state. Selecting all its anchors (`select_all_anchors`) was rejected:
  then the first drag after the handoff would move the whole path, which
  is the Select tool's job. If a later story limits node display to the
  selected paths, the handoff passes the path id into a "path, no nodes"
  `NodeSelection` state. The UX notes leave this open, and the default
  above holds. The first click of the double-click is an ordinary select
  with no movement, so it writes nothing.

- **2026-10-05: Select is the launch tool for New and for Open (AC 13).**
  `Session::new` currently defaults to Pen and `Session::open` to Node. Both
  become `Tool::Select`, and the two doc comments that justify Pen are
  replaced.

- **2026-10-05: the crate boundary.**
  - `vecmanf-document-core`: `ObjectSnapshot::translated`, the primitive
    frame box (moved from `render-core`), `translate_objects`,
    `delete_objects`. `ViewTransform` is unchanged.
  - `vecmanf-geometry-core`: `segment_bounds`.
  - `vecmanf-ui-core`: `viewport` (`Zoom`, pan gesture, zoom about a point,
    resize), `select_tool` (click, shift-toggle, drag, delete, double-click
    outcome), `ObjectSelection` (renamed), `hit_test_object` and its shared
    helper, `object_bounds`.
  - `vecmanf-render-core`: screen-space display tolerance, selection box
    rectangles taken from the decoration input, and the minimum displayed
    stroke width if the `ux-engineer` adopts it (flag 2).
  - `vecmanf-editor-wasm`: `Tool::Select`, a `session/select.rs` glue module
    (as `session/shapes.rs` is for the shape tools, so `session/mod.rs`
    stays one responsibility), screen-pixel pointer methods, `wheel`,
    `double_click`, `zoom_percent`, a resize that also renders, and the
    f64 origin subtraction in `gpu.rs`.
  - `frontend/`: the Select button and the `S` key, the non-passive wheel
    listener, Space tracking, the zoom readout in the status bar. No
    conversion arithmetic.

## Flagged to the lead

1. **AC 7's "physical millimetre at the display's reported DPI" cannot be
   verified in a webview.** A webview exposes CSS pixels (96 per inch of the
   OS's logical DPI) and no physical DPI at all. Implemented as
   `100 % = 96/25.4 CSS px per mm`, which is Inkscape's uncalibrated 1:1. On
   most monitors this does not measure as 1 mm with a ruler. The ranges
   AC 7 derives from it (0.1 mm → 30 px at 8000 %, a 600 × 400 mm sheet →
   45 × 30 px at 2 %) hold as stated. **The PO should reword it to the
   CSS-pixel definition. Default if no answer: implement the CSS-pixel
   definition. A calibration setting is a later story.**
2. **At 2 %, the document is invisible.** The default 0.25 mm stroke renders
   0.019 px wide. With no multisampling, sub-pixel triangles drop out, so
   AC 7's "overview a full sheet" shows an empty sheet. Options: (a) draw
   every document stroke at least 1 screen px wide, for display only (the
   stored width is untouched); (b) leave it as is, so thin strokes vanish
   when zoomed out. **Recommendation (a). Default: (a), unless the
   `ux-engineer`'s notes say otherwise.** It interacts with
   `stroke-and-fill-styling`'s width preview, so that slice inherits the
   rule.
3. **AC 11 is a shipped bug, not new work.** Slice 2's requirement 2
   ("reconfigure … then render, all in one frame") was built without its
   last step. It is fixed in this slice (see the resize decision).
4. **Pinch arrives differently in each webview** (AC 6). WebView2 on
   Windows sends Ctrl+wheel. WKWebView on macOS sends non-standard
   `gesture*` events. WebKitGTK is unverified. A touchscreen pinch is two
   pointers and needs its own tracking. The tester must check pinch on each
   OS. Ctrl+wheel alone does not prove AC 6.
5. **No AC conflicts with any ADR or earlier slice. No new crate is needed.**
   The UX notes (2026-10-05) fit every decision above: one box per selected
   object comes from `object_bounds`, and the Select hover box uses the same
   rule. They do not settle flag 2 or Delete inside the shape tools.
   Both have safe defaults: flag 2 (a), and Delete stays Select- and
   Node-only as specified. The specification is therefore set to
   **Ready**.
