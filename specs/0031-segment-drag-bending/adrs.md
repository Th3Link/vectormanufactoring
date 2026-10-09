# ADRs for "Segment drag bending"

A bend writes two existing registers per anchor (`handle_in`, `handle_out`)
through one new labelled command. **No new crate, no new dependency, no trait,
no generic, no new ADR, no ADR amendment, no `format_version` bump** (no new
register, no new value a version-N reader would misread). Reference state:
`main` at `909a2cc`.

## Depends on

- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one interaction is one commit; the bend is `bend_segment` (criterion 18).
  The command resolves every id before its first write; a refusal writes
  nothing.
- [ADR 0002 §3](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  `Point`, `Vec2`, `Length`, `Tolerance`; every geometric comparison takes
  an explicit tolerance.
- [ADR 0003 §1](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md)
  and [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md):
  math that needs to know what a cubic Bézier is lives in
  `curvyo-geometry-core`.
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md):
  the drag, its grab parameter and the hover are ephemeral; handles are LWW
  registers, so a click and a zero-displacement release write nothing.
- [ADR 0001 §3 to §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  rules in `ui-core`, `render-core` draws data, `editor-wasm` has no logic.
- [`specs/0002-path-node-editing/adrs.md`](../0002-path-node-editing/adrs.md):
  relative handles, zero vector = retracted handle, "commands carry resolved
  geometry, never geometric intent", one resolving function for preview and
  commit.
- [`specs/0006-path-merge-split-and-node-types/adrs.md`](../0006-path-merge-split-and-node-types/adrs.md):
  `resolve_handle_pair` is the one per-kind handle rule (criterion 12).
- [`specs/0009-unified-object-editing/adrs.md`](../0009-unified-object-editing/adrs.md)
  and [`specs/0010-edit-interaction-polish/adrs.md`](../0010-edit-interaction-polish/adrs.md):
  blue new over black old, the 3 px dead zone (`DragOrigin`), Shift axis lock
  (`MoveDrag::resolve`), Escape steps.

## Feature-local decisions

- **2026-10-10: the bend formula is one pure function in
  `curvyo-geometry-core/src/segment.rs`.** Next to
  `nearest_point_on_segment` and `subdivide_at_parameter`, with the same
  input shape (the two anchors' own points and facing relative handles), so
  `ui-core` passes `AnchorSnapshot` fields straight through:

  ```rust
  pub struct BentHandles { pub start_handle_out: Vec2, pub end_handle_in: Vec2 }
  pub fn bend_segment_handles(start: Point, start_handle_out: Vec2,
      end_handle_in: Vec2, end: Point, grab_t: f64, displacement: Vec2)
      -> Option<BentHandles>
  ```

  Rule, exactly criteria 7, 8, 10, 11: both handles shorter than
  `LINE_HANDLE_TOLERANCE` (a `Tolerance` of 1e-9 mm) make the segment a line,
  promoted to P1 = P0 + (P3-P0)/3, P2 = P0 + 2(P3-P0)/3; otherwise the stored
  P1, P2. t = clamp(grab_t, 1/6, 5/6), k = 1/(3t(1-t)), P1' = P1 + kd,
  P2' = P2 + kd, returned relative to their own anchors. `None` for a
  zero-length line (chord and both handles within the tolerance, criterion 6).
  `grab_t` stays a plain `f64` like the existing `t` parameters (a
  dimensionless parameter, not a unit). `t0` comes from one
  `nearest_point_on_segment` call at the press, on the segment the hit test
  returned (as `insert_at` does); `Hit::Segment` stays as it is (it derives
  `Eq`, an `f64` field would break it). Tests: criteria 8 to 10's numbers as a
  table, plus a proptest: for t0 in [1/6, 5/6] the bent curve passes through
  the old point at t0 plus d within 1e-6 mm.

- **2026-10-10: one new command, `Document::bend_segment`, label
  `"bend_segment"`.** Rejected: two `set_handle` calls (two commits, breaks
  criterion 18); a generic `set_handles(&[...])` (no second caller, and the
  label must name the interaction).

  ```rust
  pub fn bend_segment(&self, path: NodeId, start: AnchorId, end: AnchorId,
      start_handle_out: Vec2, end_handle_in: Vec2) -> Result<(), PathEditError>
  ```

  It resolves both ids and the directed adjacency (`end` follows `start`, or
  wraps when closed) before any write, else `NotAnAdjacentSegment`. Per anchor
  it applies `resolve_handle_pair(kind, slot, value, in, out)` (Out for
  `start`, In for `end`) and writes exactly what `set_handle` writes: the
  named handle, plus the opposite one for Symmetric and Asymmetric, never for
  Corner. That covers criterion 12. The dangling handle of an open path's end
  node is written by the same rule (invisible, and identical to a hand drag of
  that handle). One commit. It lives in a **new module
  `curvyo-document-core/src/segment_bend.rs`** (`impl Document`), not in
  `paths.rs` (1463 lines, also touched by 0017).

- **2026-10-10: the closing segment of a two-node closed path becomes
  reachable.** Criterion 12 says a two-node closed path has two segments.
  `render-core` draws both, but `ui-core::hit_test::segment_pairs` emits the
  wraparound only for `len > 2`, and `document-core::adjacent_segment_indices`
  folds (B, A) onto (A, B). Fix in this slice: `segment_pairs` uses
  `len >= 2`, and `adjacent_segment_indices` tries the given direction first,
  then the reverse. No other caller changes behaviour. The fix also corrects
  `object_bounds`, `oriented_box` and `hit_test_object` for this shape, and
  `set_segment_line`/`set_segment_curve` on that segment.

- **2026-10-10: the drag lives in `ui-core`, in a new module
  `curvyo-ui-core/src/segment_bend.rs`.** `node_tool.rs` is already past the
  module limit (about 780 production lines). `NodeTool` gets one
  `Drag::Bend(SegmentBend)` variant and one `LiveNodeDrag::Bend` variant.
  `SegmentBend` holds a `DragOrigin` (reused: the 3 px dead zone, latched,
  displacement from the press point), `t0`, and the press-time window of at
  most four anchors (previous, A, B, next; previous and next only when they
  exist) with kinds and handles. Each move is O(1) and independent of the
  path's node count (criterion 17). `resolve(cursor, shift) ->
  Option<BendResolution>` is the one function the preview and the release
  both call: Shift via the Select tool's axis lock, made `pub(crate)` in
  `select_tool/move_drag.rs` and called, not copied. Then
  `bend_segment_handles`, then `resolve_handle_pair` for both anchors. It
  returns the live `(AnchorId, handle_in, handle_out)` of A and B, plus the
  preview anchors. The preview is the dragged segment, plus the previous or
  next segment only where `resolve_handle_pair` changed the far handle; for
  a two-node closed path whose far handles changed, the closed pair.
  `pointer_up(document, point, shift)`: no commit when the dead zone was
  never left, when `|d| < 1e-9 mm` (criterion 19) or when the formula returns
  `None`. Otherwise one `bend_segment` call. The segment stays selected.
  `cancel_drag` already covers Escape.

- **2026-10-10: hit test unchanged in order.** The code's rule (the nearer of
  node and handle, a handle wins an exact tie, then the nearest segment
  within 4 px) is the one 0002's architect review fixed. Criterion 1's
  "handle, then node, then segment" describes it loosely; nothing changes.
  The bend starts only from `PointerDownOutcome::Segment`. Compound paths and
  primitives never reach it, because `Session::paths()` already filters both
  out.

- **2026-10-10: render data.** `render-core/src/decorations.rs` only.
  `Hovered` gains `Segment(NodeId, AnchorId, AnchorId)`. The session maps
  `Hit::Segment` to it instead of dropping it, and only with no drag in
  flight. It is drawn by the existing `selected_segment_overlay` code,
  parameterised by colour (`theme::ACCENT_HOVER`), **before** the nodes, and
  skipped when the same segment is selected. `DecorationInput` gains
  `handle_nodes: Vec<(NodeId, AnchorId)>`: nodes whose handles are drawn in
  the idle look without being selected (A and B during a bend, criterion 16).
  The overlay width ("2 px" in criterion 15, while the selected overlay is
  stroke width + 2 px) is the ux-engineer's call; the code takes it from
  `theme`.

- **2026-10-10: blue and black in `editor-wasm/src/session/draw.rs`.** During
  a bend the artwork is drawn from the committed `objects` (black old), not
  from the live paths. The decorations use the live paths, so A's and B's
  handles move. The blue line is a `PathSnapshot` built from
  `LiveNodeDrag::Bend`'s preview anchors (the path's id and style, no extra
  subpaths), passed to the existing `build_live_edit_preview`. It is drawn
  after the artwork and before the decorations, so nodes and handles stay on
  top. No new `render-core` preview function.

- **2026-10-10: performance.** The bend's own work per move is O(1) (above).
  The frame cost is the existing whole-document redraw. `Session::objects()`
  reuses its `drag_objects` snapshot for every Node-tool drag
  (`self.node.drag_in_flight()`), as it already does for a Select drag (same
  assumption: no remote merge during a drag, `docs/technical-debt.md`,
  "Canvas performance"). That removes the per-frame document read. No hover
  hit test runs during a drag. No spatial index: `hit_test_segment` gets a
  control-polygon bounding-box reject (inflated by the tolerance; plain min
  and max, no curve evaluation) before `nearest_point_on_segment`. Budget, as
  an `#[ignore]` benchmark in `curvyo-editor-wasm/tests/` with one
  5000-node path, release build, host CPU:
  - bend frame `draw_list` ≤ 12 ms (headroom under the 20 ms of 50 fps for
    the WebKitGTK upload);
  - one hover hit test ≤ 2 ms, document read excluded.

  If the frame misses the budget, the fix is the draw-list cache already
  described in technical debt, in its own `chore/` PR, not a spatial index.

- **2026-10-10: format impact: none.** No register, tag, or `document.json`
  field changes. Bent handles are ordinary handle values.

- **2026-10-10: overlap with `0017-style-panel-rework`
  (`story/style-panel-rework`, in progress).** Both slices touch the same
  four crates, so under `CLAUDE.md` §4 they do not run in parallel. Files both
  change: `editor-wasm/src/session/draw.rs`, `editor-wasm/src/session/mod.rs`,
  `document-core/src/path_codec.rs` (0017: 2 lines), and the `lib.rs` export
  lists of `document-core` and `ui-core`. All small, rebase-level conflicts.
  0031 does not touch `hit_test_object.rs`, `glyphs.rs`, `artwork.rs`,
  `paths.rs` or any style file. Order: 0031's branch starts from `main` after
  0017 merges.

- **2026-10-10: PR plan.** One branch `story/segment-drag-bending`, one PR,
  five milestones, each leaving the gate green:
  1. `geometry-core`: `BentHandles`, `bend_segment_handles`, table and
     property tests (criteria 7 to 11).
  2. `document-core`: `segment_bend.rs` with `bend_segment`; the directed
     `adjacent_segment_indices` (criteria 12, 18, 19; commit count and
     label).
  3. `ui-core`: `segment_bend.rs`, `Drag::Bend`, `LiveNodeDrag::Bend`,
     `segment_pairs` `len >= 2`, bounding-box reject, `pub(crate)` axis lock
     (criteria 1 to 6, 13, 19, 20).
  4. `render-core` + `editor-wasm`: `Hovered::Segment`, `handle_nodes`, the
     draw order, the `drag_objects` reuse, the benchmark (criteria 14 to 17,
     21).
  5. Frontend: no change expected (pointer capture and Escape already cover
     Node-tool drags; verify criterion 14). Tester and ux-engineer review.

## Flagged to the PO (rewording, defaults taken)

1. Criterion 3 says "3 px or more"; the shared `DragOrigin` starts a drag at
   "more than 3 px". Default: reuse `DragOrigin`. Change the wording to
   "more than 3 px".
2. Criterion 13: on an exact tie, the Select tool keeps the previous axis
   (x when there is none). The criterion says x. Default: the Select rule, one
   rule for both tools; the only difference is an exact tie after an earlier
   y.
3. Criterion 1's hit order wording (see the hit-test note above).
4. Criterion 12's "no other side" for open-path end nodes: the stored
   dangling handle is still written by the per-kind rule, as in a hand drag.
