# Plan for multi-object transform

One PR, built as four milestones on `story/multi-object-transform` (the PR split of
`adrs.md` decision 6, as commits; each milestone leaves the gate green).

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

- [ ] 1. Split `session/select_view.rs` (pure move). (ADR 6)
- [ ] 2. `group_box.rs`: bounds of the outlines, kinds, centre handle layout. (AC 1-3, 6-8, 10, 13, 14)
- [ ] 3. `classify_press`: group handle step after Alt; centre handle; rest unchanged. (AC 16, 43, 45)
- [ ] 4. Hover, cursor, hint for the centre handle; axes through the group box centre. (AC 17, 38, 46)
- [ ] 5. `render-core`: group box, degenerate line and point, member boxes (500 cutoff, 6 px,
  viewport, coincident edges). (AC 2, 4, 5, 8, 13)
- [ ] 6. `editor-wasm` decoration input; typed move and key M for a selection. (AC 35, 37)
- [ ] 7. Tests: ui-core, render-core, editor-wasm; save/reopen. (AC 50-52)

### M2 Scale (AC 9-15, 18-24, 29-32, 34, 37 (S), 38, 39, 48, 49)

- [ ] 8. `document-core`: `scaled_along`, `transform_objects`. (AC 20, 31)
- [ ] 9. `group_transform.rs` and `group_drag.rs`: scale; one resolving function. (AC 18-22, 24, 29, 32)
- [ ] 10. Resize handles in the layout, tiers, uniform-only, degenerate boxes. (AC 9-15, 21)
- [ ] 11. Live edit, readout, pivot marker, hint chips, size chip and key S. (AC 23, 30, 34, 38)
- [ ] 12. Benchmarks (`#[ignore]`) for 200 and 10,000 objects. (AC 48, 49)

### M3 Rotate (AC 25-28, 33, 37 (R))

- [ ] 13. Rotate in `group_transform.rs` / `group_drag.rs`; rotate handles; turned preview box. (AC 25-28)
- [ ] 14. Angle chip (Δ) and key R. (AC 33, 37)

### M4 Skew (AC 36, 37 (K), 40-42)

- [ ] 15. `sheared_along`; skew handles for paths only; skew gesture, guide, readout. (AC 40-42)
- [ ] 16. Skew chip and keys K / Shift+K; hint "Skew works on paths only". (AC 36, 37)

### Closing

- [ ] 17. Docs: spec status, design-system notes, `docs/technical-debt.md` (flag 7).
- [ ] 18. Full gate (CLAUDE.md section 7 and ci.yml), PR description with review guide.

## Validation

Unit tests next to the code (ui-core, document-core), acceptance tests in
`curvyo-ui-core/tests/multi_object_transform.rs` and `curvyo-editor-wasm/tests/multi_object_transform.rs`
(one test per numbered criterion where it can be tested without a pointer), `#[ignore]`
release benchmarks for criteria 48 and 49, the browser pane for the look, and the full gate.
