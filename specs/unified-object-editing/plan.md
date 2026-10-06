# Plan for Unified object editing: one Select tool for every object and its own handles

This plan covers **PR 1** only (branch `story/unified-object-editing`, worktree
`/home/marc/workbench/vecmanf-claude/unified-editing-1`, from `origin/main` at
`0b1f8ec`): `specification.md` criteria 1 to 24, 35 without the
advanced-selection clauses, 37 and 38, in the order and with the cut line of
`adrs.md` ("the two-PR split, task order and exact cut line"). PR 2 (criteria
25 to 34, delete-and-rewire) is a separate later job and is not planned here.
PR 1 adds; it deletes nothing the shape tools use, so the shape tools keep
working unchanged next to the new Select-tool editing until PR 2 lands.

No new crate, no new dependency, no new trait, no document-model change, no
`format_version` change (stays 5), no new `vecmanf-document-core` or
`vecmanf-geometry-core` function. If the build finds it needs one of these,
stop and ask the lead.

## Affected crates/modules

- `vecmanf-ui-core`: new `param_handles.rs`, `param_edit.rs`, `param_entry.rs`,
  `select_bar.rs`, `transform_primitive.rs`, `skew_math.rs`,
  `select_tool/preview.rs`; changed `transform_handle_layout.rs`
  (`TransformHandle` becomes `EditHandle`, tier function, tolerance fields,
  param arm of the hit rule), `transform_drag.rs`, `transform_commit.rs`,
  `transform_entry.rs`, `transform_math.rs`, `select_tool.rs`,
  `select_tool/entry.rs`, `lib.rs`.
- `vecmanf-render-core`: new `live_preview.rs` (`build_live_edit_preview`);
  `select_decoration.rs` (`TransformGlyphKind::Parameter`, guide line),
  `theme.rs` (knob glyph constants, `PREVIEW_NEW`), `lib.rs`.
- `vecmanf-editor-wasm`: `session/select.rs`, `session/select_view.rs`,
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

- **Press order for one selected primitive** (criterion 35): drawn handle (any
  family, Shift or not), then a press inside the selected box starts a move,
  then an outline hit selects, then the empty canvas clears. A Shift press and a
  sole selected path keep today's order (outline toggle first); the spec text
  names one primitive only and a path box often overlaps other objects.
- **A handle that is not drawn has no hit area** (criterion 6) holds for the
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
  yield distance next to the four of the ADR. 20 px is `11.3 + 5 + 4` rounded
  down, so a knob on the diagonal of the centre glyph's corner has 3.7 px of
  clearance, not 4; the clearance property excludes the centre glyph as the
  architect specified, a separate test asserts at least 3.6 px.
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
      `vecmanf-render-core`, `Session` substitution removal (`live_objects` is
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

## Validation

- Core logic test first in `vecmanf-ui-core` and `vecmanf-render-core`; session
  behaviour in `vecmanf-editor-wasm` unit and integration tests; the frontend
  has no test runner (`docs/technical-debt.md`), so bar, switches, chips and
  cursors are checked in the Browser pane against a build served from this
  worktree.
- Property tests (`proptest`, already a dev-dependency of `vecmanf-ui-core`): the
  4 px clearance of every drawn glyph pair (rectangle: aspect, radius, rotation,
  zoom; star: every point count, ratio 0.01 to 0.99, orientation, with one fixed
  worst case), release equals preview for every handle kind, drag equals typed
  entry equals bar value.
- The gate of `CLAUDE.md` §7 plus `cd frontend && npm run build`, with `vecmanf-app`
  in the workspace.
- Shape-tool tests are not touched in PR 1; every Select-tool equivalent of a
  shape-tool test is added with the old test's numbers (ADR: "port in PR 1,
  delete in PR 2").
