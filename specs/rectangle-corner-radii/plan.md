# Plan for Rectangle corner radii: a separate radius for each corner

This plan covers **PR 1** of the two-PR split in `adrs.md` decision 12 (the
model, the format and the mechanical consumer adaptation; no visible change).
PR 2 (the editing: Link switch, per-corner drag, entry, bar, readouts, hint)
gets its own section when PR 1 is verified; it is not started.

Branch `story/rectangle-corner-radii`, worktree
`/home/marc/workbench/vecmanf-claude/corner-radii`, from `origin/main` at
`e285c2d` (`CURRENT_FORMAT_VERSION` 5).

No new crate, no new dependency, no new trait. The one model change is four
registers and a `format_version` bump (decided in `adrs.md` decisions 1 to 3;
no customer question).

## Affected crates/modules

- `curvyo-document-core`: new `shape_radii.rs` (the radius commands moved out
  of `shapes.rs`), `corner_radii.rs` (`Corner`, `CornerRadii`,
  `effective_corner_radii`, `SHARP_CORNER_EPSILON_MM`), `corner_radii_codec.rs`
  (the Loro keys, legacy fallback, per-register write); changed `shapes.rs`,
  `shape_codec.rs`, `primitive_model.rs` (`Shape::Rect { bounds, corner_radii }`),
  `primitive_outline.rs`, `document.rs` (version 6, `document.json`),
  `lib.rs`; new fixtures in `tests/fixtures/`.
- `curvyo-ui-core`: mechanical adaptation (`param_handles.rs` re-exports
  `Corner`, `param_edit.rs`, `param_entry.rs`, `select_bar.rs`,
  `select_tool/bar.rs`, `transform_primitive.rs`, `transform_commit.rs`,
  `rectangle_tool.rs`, tests). The UI still writes all four corners from one
  value.
- `curvyo-editor-wasm`: `session/select_view.rs`; tests.
- `curvyo-render-core`: test fixtures only (`shape_preview.rs`).
- Docs: `CURRENT_FORMAT_VERSION` doc paragraph; `specs/index.md` untouched
  (owned by the lead).

## Tasks

- [x] 1. Pure move: the radius commands (`set_corner_radius`,
      `set_corner_radii`, `resize_rect`) from `shapes.rs` to `shape_radii.rs`;
      no behaviour change (fulfils AC 17-21 structurally, size rule of
      CLAUDE.md §5).
- [x] 2. `corner_radii.rs`, test first: `Corner` (moved from ui-core),
      `CornerRadii`, `effective_corner_radii` (CSS factor, unchanged when
      `f = 1`, floors negatives, skips zero denominators), sharp tolerance
      (fulfils AC 9, 10, 11).
- [x] 3. `corner_radii_codec.rs`, `validate_rect`, `create_rect`,
      `ALL_PRIMITIVE_KEYS`, `Shape::Rect { corner_radii }`, `document.json`,
      `CURRENT_FORMAT_VERSION` 6 with its doc paragraph; legacy fixture
      `legacy_corner_radius_v5.curvyo`, new fixture
      `corner_radii_v6.curvyo`, regenerated `future_format_version.curvyo`
      (fulfils AC 17, 18, 19, 20).
- [x] 4. `rect_outline(bounds, CornerRadii)` for 4 to 8 nodes; golden anchors
      fixture `rect_outline_mixed_radii.json`; an exact-equality regression test
      for four equal radii against a copy of the old function's expected
      output (fulfils AC 11, 16, 21).
- [x] 5. `set_corner_radius` (all four, per register guard),
      `set_corner_radii(CornerRadii)`, `resize_rect` per register; merge tests
      (different corners survive, same corner LWW, legacy node edited at two
      corners, resize Keep against a corner edit, operation counts)
      (fulfils AC 10, 12, 15 for the model side).
- [x] 6. Compile-driven adaptation of `curvyo-ui-core`, `curvyo-editor-wasm`,
      `curvyo-render-core`: every consumer reads the radii through
      `effective_corner_radii`; `resize_primitive` hands `CornerRadii` back
      (Keep) or multiplies all four by the one factor (Proportional); the UI
      writes `CornerRadii::uniform`. Behaviour identical for equal radii
      (fulfils AC 12, 14, 21).
- [x] 7. Gate (CLAUDE.md §7 plus everything `.github/workflows/ci.yml` runs),
      draft PR, CI on the head sha.

## Decisions taken while planning (inside `adrs.md`)

- Resize with Keep: the stored `CornerRadii` come back untouched, so no register
  is written; with Proportional the single factor `√(sx·sy)` is applied to all
  four stored radii and a register is written only where its value changes.
- Legacy read is per corner (own key, else `corner_radius`, else `Damaged`); a
  mistyped own key is `Damaged` and is not masked by the legacy key; a mistyped
  legacy key is `Damaged` only when some corner needs it.
- `Corner` moves to `curvyo-document-core`; `curvyo-ui-core::param_handles`
  re-exports it and keeps `Corner::local_position` as a free function there.

## Validation

- Unit tests per module (test first for `corner_radii.rs`, the codec, the
  outline, the commands), golden fixtures in `tests/fixtures/`:
  `corner_radii_v6.curvyo`, `legacy_corner_radius_v5.curvyo`,
  `rect_outline_mixed_radii.json`; `future_format_version.curvyo` regenerated
  and checked to be newer than the current version.
- Property check for the clamp: for random stored radii and sizes the effective
  radii never overlap (`f` rule) and equal `min(r, W/2, H/2)` for four equal
  radii.
- Existing tests that built `Shape::Rect { corner_radius }` change mechanically
  to `corner_radii` and keep their assertions.
- Full gate and every CI job on the exact head sha; no UI change, so no
  screenshot review.

---

# PR 2: the editing

Same branch and the same draft PR #53 (one PR for the customer). PR 1 is
verified (tester PASS, architect items A to E done). This part is the ADR's
PR 2: criteria 1 to 8, 12 to 15 (12 to 15 for rectangles with different radii),
22 and 23.

## Affected crates/modules

- `curvyo-document-core`: `Corner::opposite`, `horizontal_neighbour`,
  `vertical_neighbour` (pure helpers).
- `curvyo-ui-core`: `CornerLinking` (`transform_drag.rs`), `SelectTool`
  field and accessors, `TransformDrag::unlinked` and `ParamDragInfo`,
  `ParamValue::CornerRadius`, `value_from_pointer`/`apply_param`/
  `radius_is_limited` (`param_edit.rs`), `knob_rho` and the capped
  `param_handles`, `ParamEntry` scope and names, `SelectBarState::radius_corners`.
- `curvyo-editor-wasm`: `link_corners`/`set_link_corners`, readout texts,
  followers, entry `scope`, the "max" notice, `corner_hint_lines`, bar view
  fields.
- `frontend/`: `LinkCornersToggle` and the Radius tooltip in
  `SelectToolbar.tsx`, the scope row in `TransformEntryChip.tsx`, the knob
  hint lines in `HandleHintChip.tsx`, state in `useEditorSession.ts`.
- Docs: the "Select bar layout" width in `docs/design-system.md`.

## Tasks

- [x] 8. Part 1 review items A to E (fixture writer, sharp epsilon, outline
      helper, non-finite refusal, version pins).
- [x] 9. `Corner` helpers; `CornerLinking`, Shift as an exclusive or frozen at the
      press, `ParamValue::CornerRadius`, per-corner limit
      `min(W - r_h, H - r_v)`, writes of the three neighbours at their effective
      values from a shrunk rectangle (AC 2, 3, 4, 5, 10).
- [x] 10. `knob_rho` diagonal cap, clearance property over four independent
      radii (AC 1).
- [x] 11. Typed entry per scope, accessible names, scope row, "max" notice
      (AC 6).
- [x] 12. Mixed Radius field with the four values, Remove rounding independent
      of switch and Shift (AC 8, 22).
- [x] 13. Readout texts, followers, knob hint lines (AC 7, 23).
- [x] 14. Link toggle in the Select bar, tooltips, `aria-pressed`; "Select bar
      layout" width measured (AC 2, UX 1, 7).
- [x] 15. Gate, push, CI on the head sha.

## Validation

Unit tests for the value rules (`param_edit.rs`, `param_handles.rs`),
integration tests through the real tool (`curvyo-ui-core/tests/
rectangle_corner_radii.rs`) and `Session` (`curvyo-editor-wasm/tests/
rectangle_corner_radii.rs`), a clearance property test over four independent
effective radii, and a manual run in the browser (own tab): linked drag, unlinked
drag, entry with scope row, "max" notice, hint chip, toggle fill and bar width.
