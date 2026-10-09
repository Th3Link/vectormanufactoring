# Plan for Combine and Break apart (milestone M4 of the path-tools slice)

One branch, `story/path-tools`, one PR (see `specs/0031-segment-drag-bending/plan.md` for the order
of the milestones). `adrs.md` of this story (architect, 2026-10-10) is the design; this plan lists
what was built in which crate.

## Affected crates

- `curvyo-geometry-core`: `outline_touch.rs` (`touching_outlines`, `TOUCH_DISTANCE_MM`),
  `outline_nesting.rs` (`outline_nesting`, `outline_area_mm2`, `outline_has_area`).
- `curvyo-document-core`: `replace.rs` (`Document::break_apart`; the creation and the anchor-id
  check of `replace_with_path` are now private helpers both commands call).
- `curvyo-ui-core`: `combine.rs` (`path_availability`, `plan_combine`, `CombineRefusal`),
  `break_apart.rs` (`plan_break_apart`, regions of a compound path, `BreakApartRefusal`);
  `boolean.rs` shares its operand helpers; the default view inset (`viewport.rs`) is 128 px.
- `curvyo-editor-wasm`: `session/combine.rs` (the two commands), `session/refusal.rs` (the red
  outline of any rail command; the field is `command_refusal`), `wasm_combine.rs`.
- `frontend/`: the Path card (`PathCommands`, `PathGlyphs`), `CommandButton` shared with the Boolean
  card, `lib/pathText.ts`, `useRailCommands` (both cards and the one notice), the second rail
  column and everything keyed to `--rail-right` (116 px).
- CI: the `boolean-budgets` job runs the two new release tests.

## Tasks

- [x] 1. `geometry-core`: touch and nesting with the guarantees of the ADR (criteria 4, 10, 10a, 11,
  19); property tests, kernel budget on input (a): 16 ms for 1001 outlines against 500 ms.
- [x] 2. `document-core`: `break_apart` (13 to 15, 17), refusal-writes-nothing tests, label pin.
- [x] 3. `ui-core`: availability (1), `plan_combine` (3 to 6, 8 to 11), `plan_break_apart`
  (13 to 16); the ring painted by the nonzero rule with the area of criterion 4.
- [x] 4. `editor-wasm`: the commands (7, 15, 17), the refusal outline (9, 10, 12), the golden file
  `tests/fixtures/combine_ring_island.curvyo` (19), the four generated inputs of criterion 19
  (`tests/combine_interactivity.rs`: release 74 to 117 ms for Combine, 11 to 373 ms for Break
  apart, against 2 s).
- [x] 5. Frontend: the Path card in column B and `--rail-right` 116 px (1, 1a), tooltips (2),
  glyphs, notices and refusal texts (7, 9 to 11, 16, 17) with tests (`tests/pathText.test.ts`).

## Decisions made while building

- **`path_availability.combine` is the Boolean availability** (`NeedsTwo`, `OpenPaths`, `Ready`):
  Combine follows the same rule and the tooltip needs the open count, which a `bool` (the ADR's
  sketch) cannot carry.
- **A flat outline is "no area" by the unsigned fan area** (`outline_has_area`), not by the signed
  area: a figure eight whose lobes cancel has signed area 0 but encloses area, and criterion 10a
  asks for the self-crossing sentence for it, not "no area".
- **The refusal of a one-piece compound path** uses the sentence of criterion 16b without
  "Nothing was changed." (the UX row of `docs/design-system.md` adds it); the criterion wins.
- **`Session::clear_boolean_refusal`** keeps its name and now clears the outline of any rail
  command (the method is the host's, and existing acceptance tests call it).
- **The default view inset moves from 72 to 128 px** on both axes (criterion 1a); the three tests
  that pinned 72 px (`viewport/tests.rs`, `session/ruler.rs`,
  `acceptance_0015_pr2_session_tester.rs`) changed their number, nothing else.

## Validation

- Unit and session tests as above; `cargo nextest` for the whole workspace; the new release tests
  in the CI budget job.
- Browser check: select two or more shapes and press Combine; nested shapes become holes, the
  notice appears level with Combine; Break apart on the result; overlapping shapes are refused with
  the red outline; the rail at 800 x 600.
