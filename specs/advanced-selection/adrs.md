# ADRs for "Advanced selection"

This slice extends slice 4's Select tool. It adds no stored field and no
document write: a marquee, a lasso and an Alt-click cycle only change the
ephemeral `ObjectSelection`. **No new crate, no new external dependency, no
new `vecmanf-geometry-core` function, no `format_version` bump, no ADR
amendment.**

All 19 acceptance criteria can be built against the ADRs as they read today.
Three points need a decision from the PO or the lead. They are under
"Flagged to the lead", each with a default.

## Depends on

- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): the
  marquee rectangle, the lasso polyline and the Alt-click cycle are
  **ephemeral tool state**, the same category as `PenTool`'s in-progress
  path. None is an operation, none is written to `.vmf`, none is undoable.
- [ADR 0002 §2, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  every proximity and box test takes an explicit `Tolerance` derived from
  screen pixels at the current zoom.
- [ADR 0001 §3, §4](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  gesture logic lives in `vecmanf-ui-core`; the facade forwards scalar input;
  the marquee box and lasso line are drawn in the WebGL draw list.
- [ADR 0003 §1](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  anything that evaluates a curve stays in `vecmanf-geometry-core`. This
  slice evaluates curves only through the existing `nearest_point_on_segment`
  and `segment_bounds`.
- [`specs/0004-canvas-navigation-and-selection/adrs.md`](../0004-canvas-navigation-and-selection/adrs.md):
  "one object hit test, no third copy", "one bounds rule", "one object
  selection", the `session/select.rs` glue module. This slice extends each of
  them in place.

## Feature-local decisions

- **2026-10-05: the Select tool gets its own 8 px tolerance (AC 1, 2).**
  `SEGMENT_TOLERANCE_PX` in `vecmanf-editor-wasm/src/session/mod.rs` cannot
  simply change to 8: `Session::segment_tolerance()` is shared by the Node
  tool (through `hit_tolerances()`), the shape tools (`session/shapes.rs`:
  primitive outline hover/pick and `hit_test_primitive`) and the Select tool
  (`session/select.rs`: press, hover, double-click). Decided: a new constant
  `OBJECT_TOLERANCE_PX = 8.0` and `Session::object_tolerance()`, used by
  exactly the three `session/select.rs` call sites. `segment_tolerance()`
  and its other callers are unchanged, so the Node tool and the shape tools
  keep 4 px. No shared-tolerance refactor. `docs/design-system.md` gets an
  "Object hit-test tolerance, 8 px" row (ux-engineer).

- **2026-10-05: one candidate list, ordered once (AC 3–7).**
  `hit_test_object` returns only the best match today. Decided: a new
  `hit_test_objects(&[ObjectSnapshot], Point, Tolerance) -> Vec<NodeId>` in
  the same module returns every object within tolerance, nearest first, an
  exact tie topmost first (sort by distance with `f64::total_cmp`, then by
  descending z-index). `hit_test_object` becomes the first element of that
  list, so plain click and cycle cannot disagree about order. Both use the
  existing private `distance_to_object`; there is still one definition of
  "near an outline". `stroke-and-fill-styling`'s interior test must change
  this one ordering function, not add a second one (flag 2).

- **2026-10-05: the Alt-click cycle is ephemeral `SelectTool` state.**
  `SelectTool` gains `cycle: Option<ClickCycle>` holding the document point
  the cycle started at, the candidate ids captured at that moment, and the
  current index. Rules:
  - A plain click that hits records a cycle at index 0. Shift-click,
    a miss, a marquee, a lasso, Escape and a tool switch drop it.
  - An Alt-click within `object_tolerance()` of the cycle's start point
    advances the index modulo the candidate count and selects that
    candidate, replacing the selection (AC 4, 5, 7). Ids that no longer
    exist are filtered out first.
  - An Alt-click farther away starts a new cycle at index 0 (AC 6). The
    reset radius is the same `Tolerance` as the hit test, which is the 8 px
    AC 6 names, so it needs no constant of its own.
  - The candidate list is captured, not recomputed, so the order stays
    stable while the pointer jitters inside the radius.
  - **An Alt-click resolves on release without movement, not on press.**
    Resolving on press would replace the selection before a Ctrl+Alt lasso
    could add to it (AC 18). An Alt press therefore only records a pending
    Alt gesture.

- **2026-10-05: the Select tool's drag state becomes one enum of gestures.**
  `SelectDrag` grows from `None | Moving` to `None | Moving | PendingEmpty |
  PendingAlt | Marquee | Lasso`. A press on empty canvas (no Alt) is
  `PendingEmpty`; release without movement clears the selection (slice 4
  AC 15, unless Shift) and movement starts a `Marquee` (AC 8). An Alt press
  anywhere is `PendingAlt`; release without movement is a cycle step and
  movement starts a `Lasso` (flag 3). Modifiers are read on every event:
  the marquee's colour and mode follow the current Shift state and drag
  direction (AC 14), and Ctrl at release decides add versus replace. The
  enum is no longer `Copy` because `Lasso` holds a `Vec<Point>`.
  `select_tool.rs` is already near 600 lines; the implementer moves its
  tests to `tests/` or splits the marquee and cycle state into their own
  modules to stay under the §5 limit.

- **2026-10-05: the marquee is box arithmetic in `vecmanf-ui-core`
  (`marquee.rs`).** The module holds `MarqueeMode { Touch, Contain }`, the
  mode rule (rightward, i.e. `current.x > start.x`, is Contain; Shift swaps)
  and `objects_in_marquee(&[ObjectSnapshot], rect, mode) -> Vec<NodeId>`
  over `object_bounds`. Touch is "boxes overlap"; Contain is "box inside
  rect". Both compare with an explicit `Tolerance` (§5). The geometry-core
  question is answered "no new function": `object_bounds` is already tight
  to the curve extrema (`segment_bounds`), so Contain is **exact** for
  paths (a curve lies inside an axis-aligned rectangle exactly when its
  tight box does). Touch is the bbox approximation the specification
  accepts. A primitive uses its frame box, as the Select tool draws it (a
  star or polygon's circumscribed frame is larger than its outline; this
  matches "the box the user sees"). Document x and screen x have the same
  sign (`ViewTransform` is scale plus origin), so the direction test can run
  on document points.

- **2026-10-05: the lasso resolves on release (AC 16–19).** The polyline is
  collected in document space on every pointer move and drawn live; nothing
  is hit-tested during the drag (AC 16 asks for the line only, and Inkscape
  resolves on release). On release, `hit_test_objects_along(&[ObjectSnapshot],
  &[Point], Tolerance) -> Vec<NodeId>` in `hit_test_object.rs` resamples the
  polyline at a spacing of at most the tolerance and runs the existing
  `distance_to_object` at each sample. That is AC 17's wording ("the same
  outline-proximity test, evaluated along the whole line"), and every real
  crossing is caught because a crossing lies at most half a spacing from a
  sample. An object's `object_bounds` widened by the tolerance prefilters
  the samples, so the cost is proportional to samples near each object, not
  samples × all segments. Rejected: an exact line-versus-cubic distance in
  `geometry-core`. `kurbo` has line intersection but no segment-to-cubic
  distance, and that distance is what AC 17 asks for, so it would be a new
  kernel function with no better result at an 8 px tolerance. If the
  ux-engineer later wants a live "would be selected" highlight, the same
  function run on only the newly appended samples each frame is cheap. It
  is not built now (YAGNI).

- **2026-10-05: Ctrl composes with `ObjectSelection` through one new method.**
  `ObjectSelection` gains `extend(&[NodeId])` (append, skip ids already
  present, keep order) next to `select_single` / `toggle` / `clear`, and a
  `replace(&[NodeId])`. Ctrl means add, never toggle (spec "Open
  questions", default (a)). The marquee and lasso never move objects
  (AC 15) because neither state is `Moving`.

- **2026-10-05: input crossing the facade.** `pointer_down` today takes
  `shift`; `pointer_hover` and `pointer_up` take `constrain`
  (`ctrlKey || metaKey`). The Select tool needs Shift, Ctrl and Alt on all
  three. The wasm methods take them as separate `bool`s (ADR 0001 §4:
  scalars). The frontend keeps mapping Cmd to Ctrl on macOS as it already
  does for `constrain`, and re-sends the last pointer position when Shift
  goes down or up during a drag, so AC 14's colour updates without pointer
  movement. That is input translation, not logic.

- **2026-10-05: the crate boundary.**
  - `vecmanf-geometry-core`: no change.
  - `vecmanf-document-core`: no change.
  - `vecmanf-ui-core`: `hit_test_objects`, `hit_test_objects_along`,
    `marquee` (mode rule, box tests), the `ClickCycle` and new `SelectDrag`
    variants in `select_tool`, `ObjectSelection::extend` / `replace`.
  - `vecmanf-render-core`: `SelectDecorationInput` gains an optional marquee
    rectangle with its mode and an optional lasso polyline; styling per the
    UX notes. Draw list still depends on the view's scale only.
  - `vecmanf-editor-wasm`: `OBJECT_TOLERANCE_PX`, `object_tolerance()`, the
    modifier parameters, glue in `session/select.rs` (not `session/mod.rs`,
    which is already over 1,200 lines).
  - `frontend/`: modifier forwarding, Shift re-send during a drag.

## Flagged to the lead

1. **Click versus drag threshold (AC 4, 8).** AC 8 starts a marquee when the
   pointer "moves at all". With no threshold, 1 px of hand jitter turns an
   Alt-click into a lasso that selects every candidate at that point, so
   AC 4's cycle would be unreliable. Default: reuse the existing 3 px screen
   threshold (`PEN_DRAG_THRESHOLD_PX`, renamed to a shared
   `DRAG_THRESHOLD_PX`) for starting a marquee or lasso. The PO should
   reword AC 8 to "moves more than 3 px".
2. **Candidate order once fills exist.** `stroke-and-fill-styling/adrs.md`
   (2026-10-04) resolves hits **topmost first in tree order** once filled
   interiors take part. This spec's AC 3–5 order nearest first. Whichever
   slice ships second changes the one ordering function in
   `hit_test_objects`; click and cycle always share it. This spec's
   out-of-scope note already anticipates the revisit. Default: nearest first
   until fills ship, then the PO decides the combined order in
   `stroke-and-fill-styling`.
3. **Alt-drag that starts on an object.** AC 16 covers an Alt-drag that
   starts on empty canvas. Starting on an object is unspecified (Inkscape
   moves the selection in that case). Default: an Alt-drag is a lasso
   wherever it starts, because plain drag already moves and a lasso through
   a dense area usually starts on an object.
4. **`object-transform` makes the selection box oriented.** Once slice 5
   ships, `object_bounds` returns an `OrientedBox`, so the box the Select
   tool draws is no longer axis-aligned for a rotated object. Default:
   Contain tests that all four corners lie in the rectangle; Touch tests the
   axis-aligned box of the four corners. Both stay elementary arithmetic in
   `marquee.rs`. Both slices change `select_tool.rs` and `session/select.rs`,
   so they must not run in parallel (`CLAUDE.md` §4).
5. **Platform modifier collisions, for the tester.** Several Linux window
   managers (Xfce, Cinnamon, some KDE configurations) take Alt+drag to move
   the window. Firefox and Windows may react to a lone Alt release. The
   tester checks Alt-drag and Alt-click on each desktop OS and the browser.
   No product change by default; Inkscape has the same exposure.
