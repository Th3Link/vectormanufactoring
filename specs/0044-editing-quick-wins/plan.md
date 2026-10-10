# Plan for Editing quick wins: select all and keyboard nudge

Branch `story/editing-quick-wins`, one PR, three milestones (`adrs.md`, "Delivery").
Built on `main` at `798a8ad`: #80 (group box) and #84 (tabs) are open; this plan uses only
what `main` has (per-object boxes, `Document::translate_objects`, `session/keys.rs::decide`).
Where #80 or 0020 land first, the seams are named in the tasks.

## Affected crates/modules

- `curvyo-editor-wasm`: `session/mod.rs` (the object cache replaces `drag_objects`),
  `session/keys.rs` (two rows in `decide`, new outcomes), a new `session/nudge.rs` (the session
  side of a nudge run), `wasm_api` / `wasm_key.rs`-style glue for `time_ms` and the move readout,
  `tests/draw_list_cache_budget.rs` and session tests.
- `curvyo-ui-core`: new `nudge.rs` (distances, direction, continuation decision), `transform_commit.rs`
  (`offset_within_limit`, shared with `MoveEntry::resolve`).
- `curvyo-document-core`: a test that `Document::version()` changes on a remote merge (no code).
- `frontend`: the key forwarding (`timeStamp`), outcomes `select-all` / `nudge` / `hint-too-far`,
  the move readout chip, the live regions, the tooltip line.
- `docs`: `docs/technical-debt.md` (cache item), `docs/design-system.md` (key rows).

## Tasks

### Milestone 1: the object read cache (criterion 5)

- [x] 1. `#[ignore]` benchmark `tests/draw_list_cache_budget.rs`; numbers before the change at
  200 / 5,000 / 10,000 objects (AC 5).
- [x] 2. `Session::objects()` returns `Rc<[ObjectSnapshot]>` from a cache keyed on
  `Document::version()`; the drag snapshot becomes the cache's `pinned` flag (AC 5).
- [x] 3. Tests: the cache equals a fresh read after every kind of write (an oracle in
  `objects()` under `cfg(test)`, so the whole existing session suite checks it); a pinned drag
  keeps its read; `Document::version()` changes on a remote merge (AC 5).
- [x] 4. Numbers after; update the debt item.

### Milestone 2: the rules (criteria 1-3, 6-13)

- [x] 5. `ui-core/nudge.rs`: direction and Shift to a `Vec2`, the continuation decision, tests (AC 7, 9).
- [x] 6. `offset_within_limit` shared by `MoveEntry::resolve` and the nudge (AC 10).
- [x] 7. `decide` rows: Ctrl+A and the arrows, with the gate; `KeyInput::time_ms` carries the DOM time stamp; outcomes
  `SelectedAll`, `Nudged`, `Hint(TooFar)`; table tests (AC 1-3, 6, 7, 11, 13).
- [x] 8. Session: select all (view state, no commit), nudge through `commit_move`, the run state
  for the readout distance and the live-region text; session tests (AC 1, 2, 4, 8, 9, 10, 12).
- [x] 9. Benchmarks: 5,000 objects Ctrl+A drawn within 100 ms; nudge of 1,000 objects.
  Both are in `draw_list_cache_budget.rs`. On top of #80's group box Ctrl+A with 5,000 objects is
  drawn in 11.8 ms; one nudge event with 1,000 objects costs 24 ms. The frontend's reads after
  the key cost 260 ms (`select_bar_state`, `style_panel_view`; `technical-debt.md`).

### Milestone 3: the frontend (criteria 4, 9, 11, 14)

- [x] 10. Key forwarding with `timeStamp`; `preventDefault` per outcome; live regions; notice (AC 4, 10, 11).
- [x] 11. Move readout chip, 800 ms hold, millimetres whatever the display unit (AC 9).
- [x] 12. Tooltip second line; shortcut table rows in `docs/design-system.md` (AC 14).
  The "Too far" text shows in the key hint chip: the canvas notice slot of `0020` does not exist yet.

## Validation

- Full local gate (`CLAUDE.md` §7 plus every `ci.yml` step) before the PR is opened.
- The `#[ignore]` benchmarks in release; numbers go in the debt item and the PR.
- The Browser pane check waits until the lead frees it.
