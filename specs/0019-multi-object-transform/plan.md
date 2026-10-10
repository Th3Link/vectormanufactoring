# Plan for multi-object transform

One PR, built on `story/multi-object-transform` in the milestones of the PR split of
`adrs.md` decision 6 (group box and move, scale, rotate, skew). The commits are by layer
(document-core, render-core, ui-core with the session and frontend, then the renderer
change and the tests the spec supersedes): the three gestures share one resolving function
and one handle layout, so the milestone tasks below were built together, each tested by
its own tests.

## Affected crates/modules

- `curvyo-document-core`: `Document::transform_objects` (one commit for N objects),
  `PathSnapshot::scaled_along` and `sheared_along`, one error variant
  (`ObjectEditError::InvalidStrokeWidth`). No stored field, no `format_version` change.
- `curvyo-ui-core`: new `group_box.rs` (bounds, kinds, handle layout), `group_transform.rs`
  (one map applied to one object), `select_tool/group_drag.rs`, `select_tool/group_handles.rs`
  (queries), `select_tool/group_entry.rs` (typed values); small changes to
  `select_tool.rs`, `select_tool/press.rs`, `handles.rs`, `preview.rs`, `entry.rs`,
  `move_entry.rs`, `transform_handle_layout.rs` (side parameter of the hit test).
- `curvyo-render-core`: `select_box.rs` draws the group box (box, line, point) and the
  lighter member boxes.
- `curvyo-editor-wasm`: `session/group_view.rs` (decorations, cursor, hint, readout of a
  group), `session/select_view.rs` split first (pure move), key refusals, entry views.
- `frontend`: hint lines, `Δ` label of the angle chip, cursors, `wait` cursor, key hints,
  tooltip of "Object to path". Thin.

## Tasks

### M1 Group box and move (AC 1-8, 14, 16, 17, 35, 37 (M), 43, 45-47, 50-52)

- [x] 1. Split `session/select_view.rs` (pure move). (ADR 6)
- [x] 2. `group_box.rs`: bounds of the outlines, kinds, centre handle layout. (AC 1-3, 6-8, 10, 13, 14)
- [x] 3. `classify_press`: group handle step after Alt; centre handle; rest unchanged. (AC 16, 43, 45)
- [x] 4. Hover, cursor, hint for the centre handle; axes through the group box centre. (AC 17, 38, 46)
- [x] 5. `render-core`: group box, degenerate line and point, member boxes (500 cutoff, 6 px,
  viewport, coincident edges). (AC 2, 4, 5, 8, 13)
- [x] 6. `editor-wasm` decoration input; typed move and key M for a selection. (AC 35, 37)
- [x] 7. Tests: ui-core, render-core, editor-wasm; save/reopen. (AC 50-52)

### M2 Scale (AC 9-15, 18-24, 29-32, 34, 37 (S), 38, 39, 48, 49)

- [x] 8. `document-core`: `scaled_along`, `transform_objects`. (AC 20, 31)
- [x] 9. `group_transform.rs` and `group_drag.rs`: scale; one resolving function. (AC 18-22, 24, 29, 32)
- [x] 10. Resize handles in the layout, tiers, uniform-only, degenerate boxes. (AC 9-15, 21)
- [x] 11. Live edit, readout, pivot marker, hint chips, size chip and key S. (AC 23, 30, 34, 38)
- [x] 12. Benchmarks (`#[ignore]`) for 200 and 10,000 objects. (AC 48, 49)

### M3 Rotate (AC 25-28, 33, 37 (R))

- [x] 13. Rotate in `group_transform.rs` / `group_drag.rs`; rotate handles; turned preview box. (AC 25-28)
- [x] 14. Angle chip (Δ) and key R. (AC 33, 37)

### M4 Skew (AC 36, 37 (K), 40-42)

- [x] 15. `sheared_along`; skew handles for paths only; skew gesture, guide, readout. (AC 40-42)
- [x] 16. Skew chip and keys K / Shift+K; hint "Skew works on paths only". (AC 36, 37)

### Added during the build

- [x] A. Handle-less segments drawn as lines, so that the preview of a scaled or turned
  selection costs what a move does (AC 48; `docs/technical-debt.md`).
- [x] B. `Session::release_is_slow` and the `wait` cursor; `selection_announcement`
  (AC 28, 46, 49; UX U4).

### Closing

- [x] 17. Docs: spec status, design-system notes, `docs/technical-debt.md` (flag 7).
- [x] 18. Full gate (CLAUDE.md section 7 and ci.yml), PR description with review guide.

## Validation

Unit tests next to the code (ui-core, document-core), acceptance tests in
`curvyo-ui-core/tests/multi_object_transform.rs` and `curvyo-editor-wasm/tests/multi_object_transform.rs`
(one test per numbered criterion where it can be tested without a pointer), `#[ignore]`
release benchmarks for criteria 48 and 49, the browser pane for the look, and the full gate.
