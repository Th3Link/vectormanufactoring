# Plan for Edit interaction polish

The feature ships in four PRs (`adrs.md` decision 8). Each PR adds its own
tasks below; a later PR does not touch an earlier PR's tasks except to check
them off. Criteria numbers are those of `specification.md`.

No new crate, no new dependency, no new trait, no `format_version` change (it
stays 5). If a task finds it needs one of these, stop and ask the lead.

## PR 1: `story/edit-polish-orientation-escape-keys`

Part A (polygon and star orientation), Part D (Escape cascade, Split
selection), the keyboard gate, and the R and S entries of Part F. Criteria 1 to
8, 42 to 52, 54 and 55 (without M and K), 57, 60 to 62. Criteria of PRs 2 to 4
are out of scope here. Worktree
`/home/marc/workbench/vecmanf-claude/edit-polish-1`, from `origin/main` at
`450f5f3`. PR 2 (dashed box, Part E) runs in `edit-polish-2` and touches
`vecmanf-render-core` selection-box code and `set_device_pixel_ratio`, which
PR 1 does not.

### Affected crates and modules

- `vecmanf-document-core`: `primitive_model.rs` (`ObjectSnapshot::orientation`).
- `vecmanf-ui-core`: new `modifiers.rs`; `poly_star_tool.rs`,
  `transform_math.rs` (`rotate_delta_for`), `transform_drag.rs`,
  `transform_entry.rs` (three readers), `node_tool.rs` (`cancel_drag`,
  `clear_selection`, Split selects one node, hit-test tie),
  `hit_test.rs` (tie goes to a selected node), `select_tool/entry.rs`
  (`open_entry_for_key`), the three shape tools (`drag_in_flight`).
- `vecmanf-render-core`: `decorations.rs` (selected node glyphs drawn last).
- `vecmanf-editor-wasm`: new `session/keys.rs` (`escape`, `delete_selected`,
  `decide`, `key_down`), new `wasm_keys.rs`; `session/mod.rs` (`button_down`,
  `pointer_cancelled`, `escape() -> EscapeStep`), `session/shapes.rs`,
  `session/select_view.rs`, `session/transform_entry.rs`, `lib.rs`.
- `frontend/`: `useEditorSession.ts` (`onKeyDown` becomes a forwarder; the letter
  `switch` is deleted; `pointercancel` calls `pointer_cancelled`),
  `ToolRail.tsx` (tooltips), a transient hint message next to `EditHintChip`.
- Docs: `docs/design-system.md` key table is the `ux-engineer`'s; this PR notes
  it in the PR description.

### Tasks

- [x] 1. `Modifiers { shift, ctrl }` in `ui-core::modifiers` (it is not on `main`;
  `shape-creation-from-center` has not landed). (supports 4, 5)
- [x] 2. `ObjectSnapshot::orientation()` in document-core: `StarFrame.angle +
  rotation` normalised to (-180, 180], other kinds the `rotation` register;
  tests including the wrap at 180 and rotation by a delta. (1, 2)
- [x] 3. The readers move to it: typed angle prefill, start value and typed
  target in `TransformEntry`, the rotate drag readout in
  `session/select_view.rs`; the rotate-entry tests that read the register of a
  polygon or star are rewritten to the shown angle. (1, 7)
- [x] 4. `rotate_delta_for`: Ctrl rotate of a polygon or star snaps the shown
  angle (absolute); other kinds keep the relative rule; preview and release both
  go through `TransformDrag::resolve`. (7)
- [x] 5. Polygon/Star create-drag takes `Modifiers`; one `created_frame`
  computation for preview and release; Ctrl snaps; the readout carries the
  angle ("r 12.0 mm, -15°"). Session tests: A = (100, 50), B = (110, 48), Ctrl
  gives first vertex (109.85, 47.36), radius 10.20 mm; edge-on-axis table;
  an old project opens unchanged. (3, 4, 5, 6, 8, 2)
- [x] 6. Pure-move prelude: `escape` and `delete_selected` from `session/mod.rs`
  into `session/keys.rs`. (no behaviour change)
- [x] 7. `SelectTool::open_entry_for_key` for `EntryKey::{Angle, Size}` with
  `shift = false`: the same entry as the double-click route, on the top-right
  corner rotate handle (R) and the bottom-right corner resize handle (S). Both
  corner handles exist at any box size (`transform_handles` always lists them),
  so the stored `entry_anchor` the ADR schedules for PR 3 is not needed for R
  and S; PR 3 adds it for M and K. (57)
- [x] 8. `decide(input, KeyState) -> KeyAction` and the gate (criterion 55) in
  `session/keys.rs`, table-driven tests first; `Session::key_down` for B, N, E,
  `*`, R, S, Delete, Backspace, Enter, Escape. M and K are ignored (no hint)
  until PR 3 gives them a chip. (54, 55, 57, 60, 61)
- [x] 9. `wasm_keys.rs` (`key_down`, `pointer_cancelled`, `selection_count`); the
  frontend `onKeyDown` forwards key, modifiers, repeat and one `dom_blocked`, the
  letter `switch` is deleted. (55)
- [x] 10. `NodeTool::cancel_drag` and `clear_selection` replace `NodeTool::escape`;
  `Session::escape() -> EscapeStep`, `button_down`, `pointer_cancelled`; the
  cascade of criterion 42 for every tool; the Node-tool Escape tests are
  rewritten to the two-step behaviour. (42 to 49, 60)
- [x] 11. Split selects one node (the new second one); hit-test tie goes to the
  selected node; selected node glyphs drawn on top; the `split_selected_on_*`
  tests and the session test are rewritten with stronger assertions. (50, 51, 52)
- [x] 12. Rail tooltips and `selection_count`; transient hint message for
  "Select one object to type a value"; the hint lines of the rotate and resize
  handles gain "or R" and "or S". (62, and the R and S part of 24)
- [ ] 13. Gate: fmt, clippy (host and wasm32 per core crate and editor-wasm),
  nextest, rustdoc, deny, banned-dependency check, `npm run build`, `tsc -b
  --noEmit`, `npm run lint`, license check, `npm audit`; the CI result of the
  head commit; the Browser-pane check of the UI.

### Validation

Everything with a rule is in Rust and tested there: `orientation` and
`rotate_delta_for` as pure functions, the create-drag golden numbers, the key
table as one parameterised test over (tool, selection, key, shift) plus every
gate condition on its own and in pairs, the Escape cascade once per tool, Split
on an open and a closed path. The frontend has no test runner
(`docs/technical-debt.md`); the forwarder, the tooltips and the hint chip are
checked in the Browser pane against a worktree build and by the `ux-engineer`
review.

## PR 2: `story/edit-polish-dashed-box`

Part E, criteria 63 to 68 (another implementer, worktree `edit-polish-2`; its
tasks go here when it lands).

## PR 3: `story/edit-polish-typed-skew-move`

Part B and the keys M and K of Part F. Tasks are added when PR 3 starts.

## PR 4: `story/edit-polish-move-copy-lock`

Part C and the Copy check of criterion 23. Tasks are added when PR 4 starts.
