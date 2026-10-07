# Plan for Shape creation from the center: Shift and Shift+Ctrl while drawing

Branch `story/shape-creation-from-center`, worktree
`/home/marc/workbench/vecmanf-claude/shape-center/`, based on `main` at
`f0e3ebb` (after `unified-object-editing` and `edit-interaction-polish`
parts 1 to 3).

## State of `main` that this plan adapts to

`specification.md` and `adrs.md` were written before two features landed. What
exists now, checked in the code:

- `vecmanf-ui-core/src/modifiers.rs` already has `Modifiers { shift, ctrl }`
  (`NONE`, `new`). Not created again.
- `Session::pointer_hover` / `pointer_up` already build `Modifiers` and call
  `shape_pointer_move(point, Modifiers)` / `shape_pointer_up(point,
  Modifiers)`, and the polygon/star tool already takes `Modifiers`. Only
  `RectangleTool` and `EllipseTool` still take `constrain: bool` and are
  passed `modifiers.ctrl` in `session/shapes.rs`.
- The frontend already forwards window-level key events to
  `modifiers_changed` and then re-sends `pointer_hover` at the last pointer
  position (`applyModifiers` in `frontend/src/hooks/useEditorSession.ts`), so
  criterion 8 reaches the shape tools through `shape_pointer_move`. No
  `wasm_api.rs` or frontend change.
- The shape tools only create (no handles, no hit test); a committed
  create-drag selects the new shape and switches to the Select tool.

## Deviations from `adrs.md` and `specification.md`

1. **`Modifiers` is not introduced here**; it is used. The ADR's "New
   `modifiers.rs`" and its `Session`-side change shrink to: the rectangle and
   ellipse tools take `Modifiers` in place of `constrain: bool`;
   `session/shapes.rs` passes `modifiers` instead of `modifiers.ctrl`.
2. **Criterion 16 and the first sentence of 17 are superseded** by
   `unified-object-editing` 25, as the spec says; nothing to build, and the
   ADR's test items for them (Shift press on an outline toggles, handle drags
   under Shift/Ctrl) are not written. What replaces them is already tested
   there: a press in a creation tool always creates. One test is added that a
   Shift press on an existing rectangle's outline in the rectangle tool starts
   a create-drag (the surviving part of 16).
3. **Criterion 17, polygon/star Ctrl.** `edit-interaction-polish` criterion 4
   (see its `adrs.md`: "`shape-creation-from-center` criterion 17
   for Ctrl only") made Ctrl snap the polygon/star create angle. So "Shift
   and Ctrl have no effect" is true only for **Shift**; Ctrl keeps its snap.
   This feature does not touch `poly_star_tool.rs`; the test asserts that
   Shift changes nothing and Ctrl still snaps.
4. **Session-level criterion 8.** `Session::modifiers_changed` only caches
   state; the shape tools read modifiers from the `pointer_hover` that the
   frontend re-sends (ADR note of 2026-10-06). Session tests call
   `pointer_hover` with the new modifiers, as the ADR says. The browser check
   covers the real key path.
5. **Tests migrated, not weakened.** `tests/acceptance_0003.rs` (tester-written)
   calls `RectangleTool::pointer_up(.., bool)`. The signature change is the
   ADR's decision, so those calls are rewritten mechanically (`false` to
   `Modifiers::NONE`, `true` to `Modifiers::new(false, true)`); no assertion
   changes.
6. **`create_drag_box` is `pub(crate)`** in `shape_tool_common.rs`
   (the ADR names the function, not its visibility); the crate's public API
   gains nothing but the changed `pointer_move` / `pointer_up` parameter type.
7. **Tolerance:** 1e-9 mm, per ADR flag 1.

## Affected crates/modules

- `vecmanf-ui-core`: `shape_tool_common.rs` (`CreateDragBox`,
  `create_drag_box`), `rectangle_tool.rs`, `ellipse_tool.rs`, tests in
  `tests/acceptance_0003.rs` (signature migration) and a new
  `tests/shape_creation_from_center.rs`.
- `vecmanf-editor-wasm`: `session/shapes.rs` (pass `Modifiers`), new
  `tests/shape_creation_from_center.rs` through `Session`.
- No new crate, dependency, `format_version`, `wasm_api.rs` or frontend
  change. Docs: `specs/shape-creation-from-center/` (status, plan);
  `docs/design-system.md` only if the tool tooltips change (they do not).

## Tasks

- [x] 1. `create_drag_box(a, b, modifiers) -> Option<CreateDragBox>` in
  `shape_tool_common.rs` with unit tests on the spec's golden numbers (fulfils
  AC 1, 2, 3, 4, 5, 6, 7, 12). Tests first.
- [x] 2. Rectangle and ellipse tools take `Modifiers`; one private
  `created_shape(down_at, point, modifiers)` is the only computation for
  `live_shape` and `pointer_up`; migrate callers and existing tests (fulfils
  AC 1 to 7, 9, 10, 11 by construction, 12, 13).
- [x] 3. Tool-level tests in `ui-core/tests/shape_creation_from_center.rs`:
  every criterion 1 to 7, 9, 10, 12, 13 through the public tool API, plus a
  proptest (Shift centre equals A; last preview equals the commit over random
  move/modifier sequences) (AC 1 to 7, 9, 10, 12).
- [x] 4. `Session` passes `Modifiers` to both tools; Session tests: readout
  strings, modifier change with the pointer still, release modifiers win,
  Escape then modifier changes, pan mid-drag, Shift press on an outline,
  polygon/star under Shift and Ctrl, saved `format_version` (AC 8, 11, 13, 14,
  15, 16, 17).
- [ ] 5. Full gate (`CLAUDE.md` section 7 plus everything `ci.yml` runs).
- [ ] 6. Check in the browser: Shift, Shift+Ctrl, Ctrl, mid-drag modifier
  changes without mouse movement; polygon/star unchanged (AC 8, 9, 11, 14).
- [ ] 7. Draft PR, CI green on the exact head sha.

## Validation

Golden numbers of the spec as unit tests; a proptest for criteria 1/9; Session
tests for the criteria that need the session (readout text, hover with the
pointer still, Escape, pan, saved version); the running app in the Browser pane
for criterion 8 with real key events and criteria 9/11 visually.
