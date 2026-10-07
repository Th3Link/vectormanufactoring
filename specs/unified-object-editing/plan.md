# Plan for Unified object editing: one Select tool for every object and its own handles

This plan covers **PR 1** (criteria 1 to 24, 35 without the advanced-selection
clauses, 37 and 38) and **PR 2** (criteria 25 to 34, delete-and-rewire), both on
branch `story/unified-object-editing` and draft PR #38, worktree
`/home/marc/workbench/vecmanf-claude/unified-editing-1`, from `origin/main` at
`0b1f8ec`, in the order and with the cut line of `adrs.md` ("the two-PR split,
task order and exact cut line"). PR 1 added and deleted nothing the shape tools
used; PR 2 (tasks 11 to 15 below) turns the shape tools into creation-only tools
and deletes the editing code that PR 1 made redundant. The two merge in one
window (the lead's decision).

No new crate, no new dependency, no new trait, no document-model change, no
`format_version` change (stays 5), no new `curvyo-document-core` or
`curvyo-geometry-core` function. If the build finds it needs one of these,
stop and ask the lead.

## Affected crates/modules

- `curvyo-ui-core`: new `param_handles.rs`, `param_edit.rs`, `param_entry.rs`,
  `select_bar.rs`, `transform_primitive.rs`, `skew_math.rs`,
  `select_tool/preview.rs`; changed `transform_handle_layout.rs`
  (`TransformHandle` becomes `EditHandle`, tier function, tolerance fields,
  param arm of the hit rule), `transform_drag.rs`, `transform_commit.rs`,
  `transform_entry.rs`, `transform_math.rs`, `select_tool.rs`,
  `select_tool/entry.rs`, `lib.rs`.
- `curvyo-render-core`: new `live_preview.rs` (`build_live_edit_preview`);
  `select_decoration.rs` (`TransformGlyphKind::Parameter`, guide line),
  `theme.rs` (knob glyph constants, `PREVIEW_NEW`), `lib.rs`.
- `curvyo-editor-wasm`: `session/select.rs`, `session/select_view.rs`,
  `session/draw.rs`, `session/mod.rs`, `session/node.rs`
  (`live_node_drag_paths` loses the Select branches),
  `session/shapes.rs` (`primitives_for_render` loses the Select branches),
  `session/transform_entry.rs`; new `session/select_bar.rs` and
  `wasm_select_bar.rs` (a second `#[wasm_bindgen] impl WasmSession`);
  `wasm_api.rs` does not grow.
- `frontend/`: `SelectToolbar.tsx`, `ScaleStrokeSwitch.tsx` (generalised to one
  `ToolbarSwitch`), `useEditorSession.ts`, `Canvas.tsx`, `lib/cursors.ts`,
  `TransformEntryChip.tsx`, `HandleHintChip.tsx`, `App.tsx`.
- Docs: `docs/design-system.md` rows are already in the spec PR (#37); only
  gaps found while building are added. `docs/technical-debt.md`,
  `specs/index.md` as needed.

## Decisions taken while planning (inside `adrs.md`)

- **Press order for one selected object** (criterion 35, amended by the PO):
  drawn handle (any family, Shift or not), then a plain press inside the sole
  selected box starts a move, then an outline hit (4 px) selects, then the empty
  canvas clears. With Shift the outline hit is tried first (Shift-click adds to
  the selection over a filled shape).
- **A handle that is not drawn has no hit area** (criterion 6, amended by the PO:
  edge resize handles under 24 px stay hit-testable) holds for the
  parameter handles and the centre handle: `drawn_edit_handles` builds the one
  list the hit rule reads, and a knob below 72 px or a centre handle that yields
  is not in it. An edge resize handle of a box under 24 px stays hit-testable
  although not drawn, as in slice 5 (`is_drawn_handle`, the tester's model in
  `acceptance_otr_tester.rs` and the "handles reachable" reverify test pin
  it); the architect's text keeps `is_drawn_handle` for the same reason. Reading
  criterion 6 literally would remove it; flagged to the lead.
- `apply_param` is the one value rule for drag, typed entry and bar field and
  returns the start snapshot unchanged when the clamped value equals the start's
  effective value within 1e-9 mm, so a drag back to its start writes nothing
  (criterion 12) even when the stored radius is larger than the effective one.
- The radius gain `G(s)` is frozen at the press (`TransformDrag::param_gain`).
- A fifth tolerance field `param_centre_yield_mm` (20 px) carries the centre
  yield distance next to the four of the ADR. The centre glyph's rounded-square extent is 10.07 px, so a knob
  20 px away keeps at least 4 px; a separate test asserts it.
- **Document reads.** `Session::draw_list` reads the document once per frame and
  a Select drag keeps the snapshot it started with (`drag_objects`): the 200
  object move frame fell from 123 ms to 8.5 ms. The architect's cache by
  document version needs a `Document` accessor that PR 1 may not add.

## Tasks

- [x] 1. Pure-move prelude, one commit, no behaviour change: `resize_primitive`,
      `scaled_star_frame`, `pin_*` to `transform_primitive.rs`; `SkewFrame`,
      `skew_frame`, `skew_angle`, `skew_factor` to `skew_math.rs`; preview
      accessors to `select_tool/preview.rs` (no AC; keeps `select_tool.rs`
      under the size limit, `docs/technical-debt.md`)
- [x] 2. Rename commit, compile-driven: `TransformHandle` to `EditHandle`,
      `DragOrigin::side_rotate_revealed` to `shift_at_press`, `EntryKind::Radius`
      to `OuterRadius` with accessible name "Outer radius" (AC 19); the `Param`
      variant arrives with its layout in task 3
- [x] 3. `param_handles.rs`: `ParamHandle`, `Corner`, `param_handles` layout
      (inset 15 px, pitch 14 px, `L(s)`, `G(s)`), tolerance fields,
      `handle_tiers` (24/48/72 with `THRESHOLD_SLACK`), `centre_drawn`, the
      "no param handle during another drag and for two or more objects" drawn
      set; tests: tier boundaries 24/48/72, the rectangle and star clearance
      property sweeps including the star worst case (4.6 px at `s` 72),
      `L(T) > 0`, named glyph-size constants (AC 1, 2, 4, 7, 8)
- [x] 4. Param arm of `hit_transform_handle`: 12 px radius, rank 0, no inner
      band, tie order param > resize > skew > rotate, undrawn handles absent from
      the hit set, body reachability, drawn glyph always hits its own handle;
      press order of criterion 35 (AC 5, 6, 35)
- [x] 5. `param_edit.rs`: `ParamValue`, `value_from_pointer` (rect radius with
      gain per corner diagonal, star ratio), `apply_param` with clamps,
      `commit_param`; `TransformDrag` and `commit_gesture` arms; dead zone,
      Escape, zero-position and clamp tests ported from the shape-tool tests
      (AC 2, 3, 4, 9, 24)
- [x] 6. `ScaleModes { stroke, radius }`, `CornerRadiusScaling { Keep, Proportional }`
      (default `Keep`), `resize_primitive` radius factor, read at the press and
      when an entry opens, `Document::resize_rect` leaves an unchanged radius
      register alone, merge test (peer A resizes with `Keep`, peer B sets the
      radius; B survives), `SelectTool` and `Session` accessors; the `0005`
      criteria 9 and 31 tests run with the switch on, new tests assert the off
      default (AC 23)
- [x] 7. `LiveEdit` and `SelectTool::live_edit`, `build_live_edit_preview` in
      `curvyo-render-core`, `Session` substitution removal (`live_objects` is
      the only substitution, for decorations), zero-offset move guard,
      release-equals-preview property test for every handle kind, `#[ignore]`
      benchmark of `Session::draw_list()` for a 200-object move (AC 10, 11, 12,
      13, 14, 15)
- [x] 8. `ParamEntry` (`param_entry.rs`), `OpenEntry` in `SelectTool`, readouts
      "r 3.5 mm" and "ratio 0.45", cursor `pointer` and hint strings for the
      parameter handles, double-click on a parameter handle opens its entry
      (AC 5, 18, 19, 20)
- [x] 9. `select_bar.rs` (`select_bar_state`, `ids_of_kind`, Radius field state
      incl. Mixed and "limited", Remove rounding enabled flag, Points and
      Ratio, Object to path), `Session` bar methods and `wasm_select_bar.rs`,
      `SelectToolbar.tsx` with the two switches first, kind groups, Object to
      path last, parameter-handle drawing and cursor in the frontend, Edit
      hint strings (AC 21, 21a, 22, 23). Remove rounding, Points, Ratio and
      Object to path stay in the shape bars until PR 2
- [x] 10. Round-trip and compatibility tests (AC 24, 38), `docs/design-system.md`
      gaps, `docs/technical-debt.md`, full gate, Browser-pane check, demo notes

### PR 2

- [x] 11. `curvyo-ui-core`: `ResizeDirection` to `resize_direction.rs`;
      `shape_tool_common.rs` becomes `CreatePreview` and `CreateOutcome` plus
      `is_degenerate` and `constrained_endpoint`; the rectangle, ellipse and
      polygon/star tools become creation-only (`pointer_down(point)`,
      `pointer_move`, `live_shape`, `pointer_up -> CreateOutcome`, `escape`); a
      press without movement writes nothing and leaves the selection alone;
      `handle_layout.rs`, `shape_hit_test.rs`, `pin_rect_resize`,
      `pin_ellipse_resize` deleted (AC 25, 26)
- [x] 12. `SelectTool::double_click` on a primitive returns `EditHint` and never
      a handoff (paths still hand off to the Node tool); a double-click on a
      parameter handle still opens its entry (AC 31, 32, 33)
- [x] 13. `curvyo-render-core`: `ShapeDecorationInput`, the shape handle glyphs
      and the primitive bounding box outline deleted; `build_primitive_strokes`
      and `build_shape_live_preview` stay (AC 27)
- [x] 14. `curvyo-editor-wasm`: `Session::shape_pointer_up` sets `Tool::Select`
      and selects the new id; creation tools draw each selected object's plain
      box, no hover, no handles; `convert_selected_to_paths` and
      `remove_corner_rounding` live in `session/select_bar.rs`; the old
      `poly_star_*` selection-editing calls leave `wasm_api.rs`;
      `double_click` returns a bool for the hint chip (AC 27, 28, 29, 31)
- [x] 15. Frontend: `ShapeToolbar` holds only the polygon/star next-shape
      settings with the "New:" prefix; `EditHintChip` (three lines); crosshair
      for every creation tool; no hover highlight (AC 29, 30, 32, 34)

#### Customer test round (2026-10-06)

Criterion 27 is reversed by the customer (the PO amends the spec): choosing a
creation tool (`Session::set_tool`, the one entry for rail, shortcuts and `*`)
clears the selection and no selection box is drawn under it; the Pen and Node
tools keep the selection. Creating a shape still hands over to Select with it
selected. During any Select-tool drag or bar slider edit no hover box of other
objects is drawn and hover does not change the cursor. Tests: `select_view.rs`
`choosing_a_creation_tool_clears_the_selection_and_draws_no_box` and
`no_hover_highlight_or_hover_cursor_while_a_select_drag_runs`; the tester's
`ac26_*` and `ac27_*` tests in `acceptance_unified_editing_pr2.rs` and the
`shapes.rs` press-without-movement test were rewritten for the empty selection.

#### What replaced each deleted test

| Deleted | Replaced by |
|---|---|
| `rectangle_tool.rs` unit tests: resize, radius drag (rotated, shrunken, zero at the diagonal), remove rounding, selection click | `tests/unified_object_editing.rs` `ported_ac3_*`, `ported_all_four_radius_handles_*` and the `param_edit.rs` tests of PR 1; the Remove rounding tests in `curvyo-editor-wasm/tests/unified_object_editing.rs` and `acceptance_unified_editing.rs`; the creation-only unit tests in `rectangle_tool.rs`; `session/shapes.rs` `a_press_without_movement_creates_nothing_and_keeps_the_selection` |
| `ellipse_tool.rs` unit tests: resize | `ported_ac9_resizing_an_ellipse_can_break_rx_eq_ry` |
| `poly_star_tool.rs` unit tests: resize, inner radius, point count, ratio preview | `ported_ac13_*`, `ported_ac14_*`, `ported_ac15_*`; the bar Points tests in `curvyo-editor-wasm/tests/unified_object_editing.rs` and `acceptance_unified_editing.rs` |
| `handle_layout.rs`, `shape_hit_test.rs` tests | the PR 1 `param_handles.rs` and `transform_handle_layout.rs` tests; `hit_test_object.rs` tests for the outline rule |
| `acceptance_0003.rs` AC3, AC9, AC13, AC14, AC15, `hit_test_handle_ignores_non_draggable_echo_handles` | the `ported_*` tests above (same numbers) |
| wasm `acceptance_0005.rs` `ac25_*` shape-tool handle tests | eight `ac25_*` tests rewritten against the Select tool with the old numbers (`radius_knob`, `inward_step` helpers) |
| `session/shapes.rs` radius and ratio preview tests | the `unified_object_editing.rs` (wasm) tests `a_star_inner_radius_drag_shows_a_ratio_readout` and the release-equals-preview tests of PR 1 |
| `acceptance_object_transform_refinements.rs` double-click tests (ui-core) | same tests, primitives now assert `EditHint`, paths still assert `Hit`; the outline-at-a-handle-spot test asserts `EditHint` and an empty entry |

No test was deleted without a replacement of at least the same strength; the
shape-tool double-click handoff tests changed their expected outcome on purpose
(criterion 32 reverses the old behaviour).

#### Deleted-names check

A grep over the Rust crates and `frontend/src` finds none of: `HandleKind`,
`ShapeHandle`, `handles_for`, `rect_handles`, `ellipse_handles`,
`polygon_or_star_handles`, `corner_inward_diagonal`, `resize_rect_bounds`,
`resize_ellipse_frame`, `scale_star_frame`, `CARDINAL_FOUR`, `ShapeHitTolerances`,
`LiveShape`, `rects_only`, `ellipses_only`, `polygons_and_stars_only`,
`apply_selection_click`, `pin_rect_resize`, `pin_ellipse_resize`,
`hovered_primitive`, `update_hovered_primitive`, `shape_matches_active_tool`,
`tool_for_shape`, `shape_decoration_input`, `primitives_for_render`,
`ShapeDecorationInput`, `RenderShapeHandle`, `ShapeHandleKind`,
`shape_handle_glyph`, `build_shape_draw_list`. Two names that look similar remain
on purpose: `hit_test::hit_test_handle` (the Node tool's own node/handle test) and
`Session::poly_star_ratio` (the next-shape setting).

#### Not done in PR 2

- `shape-creation-from-center` criteria 16 and the first sentence of 17 are
  superseded by this story; that spec is not built and not edited here.
- No frontend test runner exists, so the hint chip, the "New:" bar, the crosshair
  and the hand-over to Select are checked in the Browser pane only.

## Validation

- Core logic test first in `curvyo-ui-core` and `curvyo-render-core`; session
  behaviour in `curvyo-editor-wasm` unit and integration tests; the frontend
  has no test runner (`docs/technical-debt.md`), so bar, switches, chips and
  cursors are checked in the Browser pane against a build served from this
  worktree.
- Property tests (`proptest`, already a dev-dependency of `curvyo-ui-core`): the
  4 px clearance of every drawn glyph pair (rectangle: aspect, radius, rotation,
  zoom; star: every point count, ratio 0.01 to 0.99, orientation, with one fixed
  worst case), release equals preview for every handle kind, drag equals typed
  entry equals bar value.
- The gate of `CLAUDE.md` §7 plus `cd frontend && npm run build`, with `curvyo-app`
  in the workspace.
- Shape-tool tests were not touched in PR 1; every Select-tool equivalent of a
  shape-tool test was added with the old test's numbers (ADR: "port in PR 1,
  delete in PR 2"), and PR 2 deleted the old ones (table above).
