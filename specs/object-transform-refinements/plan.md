# Plan for Object transform refinements: handles, pivots, numeric entry, 22.5° snap

Branch `story/object-transform-refinements`, worktree
`/home/marc/workbench/vecmanf-claude-object-transform-refinements`, from
`origin/main` at `e67dea7`. No new crate, no new dependency, no
`format_version` change (stays 5), no `curvyo-geometry-core` change.

Decisions taken while planning (inside the architect's `adrs.md`):

- The 3 px dead zone is sticky: once a drag's pointer has left the radius
  (seen on a move, or on release) the drag is active for its remaining
  life; the delta is always from the press point. State lives in the
  `SelectDrag` variant (`DragOrigin`), threshold in `TransformHandleTolerances`.
- Hit-testing of edge resize handles stays slice 5's (not hidden by the
  24 px "drawn" tier); everything new is only hit-tested when drawn.
- `TransformHandleTolerances` gains the px-derived mm values (skew radius,
  skew offset, centre hover, tier thresholds, dead zone) with one
  `at_scale(px_per_mm)` constructor in `ui-core`, so the screen-pixel numbers
  from `docs/design-system.md` live in one place.

## Affected crates/modules

- `curvyo-document-core`: `path_model.rs` (`PathSnapshot::sheared`).
- `curvyo-ui-core`: new `angle_snap.rs`, new `transform_entry.rs`;
  `transform_math.rs` (pivot, snap, skew arithmetic, size-to-delta),
  `transform_handle_layout.rs` (handle enum, positions, tiers, one hit rule),
  `transform_drag.rs` (`resolve` per gesture, `compute_skew`),
  `select_tool.rs` (dead zone, modifiers, entry, double-click rule).
- `curvyo-render-core`: `select_decoration.rs`, `glyphs.rs`, `theme.rs`
  (glyph kinds, skew guide, tokens).
- `curvyo-editor-wasm`: `session/select.rs` (+ new
  `session/transform_entry.rs`), `wasm_api.rs`.
- `frontend/`: `useEditorSession.ts`, `Canvas.tsx`, `lib/cursors.ts`, new
  `TransformEntryChip.tsx`, `HandleHintChip.tsx`.
- Docs: `docs/design-system.md` (only where the build found a gap),
  `specs/index.md` status, `docs/technical-debt.md` (session split done).

## Tasks

- [x] 1. Pure-move split of `session/mod.rs` and `transform_handle_layout.rs`
      (no AC; commit `db8ff55`)
- [x] 2. `angle_snap.rs`: `snap_angle` over {k·15°} ∪ {k·22.5°}, nearest
      wins, ties toward the start, all quadrants, both signs; skew cap ±75°
      (AC 33, 34, 36, 47)
- [x] 3. `PathSnapshot::sheared(pivot, ku, kv)`: anchors and handle vectors,
      `rotation` untouched, exact inverse (AC 38, 42, 43, 44, 46)
- [x] 4. `transform_math`: `rotate_pivot(box, grabbed, shift)` (opposite
      corner/side), `rotate_delta_angle` snaps with `snap_angle`, skew
      factor and angle, `local_delta_for_size`, polygon/star radius delta
      (AC 12-15, 16, 38, 39, 47)
- [x] 5. Handle layout: `TransformHandle::{Resize, Rotate, Skew, Move}`,
      positions of 4 corner rotate, 4 Shift side rotate, 4 skew (paths only,
      per-axis tier), centre (tier 48 px), single nearest-centre hit rule
      with tie order resize > skew > rotate, hover-only centre
      (AC 1, 4-11, 37, 48, 50, 53)
- [x] 6. `transform_drag`: split into `local_delta_of` /
      `resize_by_local_delta`, `rotate_by`, `compute_skew` (primitives
      returned unchanged), one `resolve` shared by preview, release and entry
      (AC 16, 27, 38-41, 45, 46, 51, 52)
- [x] 7. `SelectTool`: 3 px dead zone on every drag, `Rotating(direction)`,
      `Skewing`, frozen handle set during a drag, Shift-reveal state via
      `modifiers`, Shift-press on a side rotate handle is a rotate not a
      toggle, `live_*` previews, Escape (AC 3, 6, 10, 11, 14, 41)
- [x] 8. `transform_entry.rs` (state, parser, linking, unchanged rule,
      outcome), `SelectTool::open_entry` / commit / cancel, double-click
      dispatch table incl. handoff rule inside the box and no handoff on
      handles (AC 3, 18-23, 25-32, 49)
- [x] 9. `render-core`: glyphs (centre move, skew arrows, rotate at 8
      positions, entry handle in dragging look), skew fixed-line guide,
      pivot preview (AC 1, 5, 6, 37, 55, 56, UX glyph table)
- [x] 10. `Session` glue and `wasm_api`: tolerances, `modifiers_changed`,
      `double_click(x, y, shift, ctrl)`, cursor hints (`move`, `skew:<deg>`),
      readouts (rotate 1 decimal, skew), entry view / commit / cancel,
      close-on-tool-switch/selection change/Delete/convert, hint query
      (AC 3, 6, 10, 18-32, 35, 40, 48, 54)
- [x] 11. Frontend: window-level modifier forwarding and blur clearing,
      double-click with second-press position and modifiers, entry chip (DOM,
      keys, blur-not-swallowed, invalid feedback), skew cursor, hint chip,
      tokens (AC 6, 18-22, 25-31, 48, 54)
- [x] 12. Rewrite slice 5 / 4 tests that press the old top rotate handle,
      rely on a sub-3-px drag, or expect an inside double-click to do
      nothing (spec "Changes to shipped behaviour" item 6)
- [x] 13. Acceptance tests for every criterion (kept in
      `curvyo-ui-core/tests/acceptance_object_transform_refinements.rs` and
      `curvyo-editor-wasm/tests/acceptance_object_transform_refinements.rs`),
      docs, PR

Optional criteria 54 (hint chip), 55 (pivot preview) and 56 (skew guide) are
built (tasks 9-11). Task 13's PR step: draft PR #35, not marked ready.

## Validation

- Unit tests next to each pure function (snap table, parser, shear inverse,
  hit rule, size-to-delta), property-style round trips: drag S vs entry S and
  drag A vs entry A equal within 1e-9 mm / 1e-12 rad for every kind, handle
  and modifier combination; skew then inverse skew within 1e-9 mm.
- Session-level acceptance tests drive `Session` with screen-derived points
  (dead zone, double-click table, Shift reveal, entry open/commit/cancel,
  undo/redo counts, save/reopen of a typed rotation).
- Full gate from `CLAUDE.md` §7, including `curvyo-app` and
  `cd frontend && npm install && npm run build`.
- Manual check in the Browser pane against a served build of this worktree.

## Known limitations

- The size entry fields are 100 px wide (design-system row): a width of 10000 mm or more (for example 12345.6) clips its last digit. Widening needs a UX decision on the field width.
