# Plan for Pen path extension (milestone M3 of the path-tools slice)

One branch, `story/path-tools`, one PR (see `specs/0031-segment-drag-bending/plan.md` for the order
of the milestones). `adrs.md` of this story (architect, 2026-10-10) is the design; this plan lists
what was built in which crate.

## Affected crates

- `curvyo-document-core`: `reversed_anchors` (`path_model.rs`), `smooth_corner_handles`
  (`smooth_handles.rs`, the Corner to Asymmetric rule taken out of `convert_anchor_kind`),
  `merged_junction` (`junction.rs`, the Join merge rule; `join_endpoints` calls it), `path_extend.rs`
  (`PathGrowth`, `extend_path`, `connect_paths`, `close_paths`), `Document::version()`.
- `curvyo-ui-core`: `closing_join.rs` (`JoinType`, `resolve_closing_node`), `pen_target.rs`
  (`EndNodeIndex`, `pen_target`), `pen_tool.rs` (`Continuation`, `PressAction`, closing preview),
  `close_path.rs` (`plan_close_paths`, `closable_counts`).
- `curvyo-render-core`: `pen_cue.rs` (target ring and glyph, dashed closing segment, closing node).
- `curvyo-editor-wasm`: `session/pen.rs` (end-node cache, press targets, cue data),
  `session/close_path.rs`, `wasm_pen.rs`.
- `frontend/`: `lib/penText.ts`, `usePenCue`, `PenHintChip`, the continue and join cursors,
  `ClosePathGroup` in the Node bar.

## Tasks

- [x] 1. `document-core`: the three shared helpers and `path_extend.rs` with tests for criteria 4, 5,
  8 to 11, 14, 19 to 21 (`tests/path_extend.rs`), labels and refusals.
- [x] 2. `ui-core`: closing join (criterion 15 with Sharp retracting the closing-side handle),
  `pen_target` (1, 7, 12, 13, 23), `PenTool` (2 to 5, 8 to 11, 14, 25), `close_path` (18 to 21);
  preview and commit share `resolve_close`.
- [x] 3. `render-core` and `editor-wasm`: the cue (1, 7, 13, 17, 22), the end-node cache (24), the
  Close path command; session tests `tests/pen_path_extension.rs`; benchmark `tests/pen_target_budget.rs`
  (warm hover 0.12 ms against 2 ms; first query after a change, which rebuilds the index, 45 ms for
  5000 paths).
- [x] 4. Frontend: cursors (22), hint chip (1, 7, 16), Close path group and notice (18, 20).
  Texts in `lib/penText.ts` with tests (`tests/penText.test.ts`).

## Decisions made while building

- **Close target needs three nodes.** The old Pen closed onto the first node with two nodes placed;
  criterion 13 says a closed path has at least three. The existing unit tests that pinned two were
  changed (`pen_tool.rs`, `session/mod.rs`).
- **The editing set of the Close path buttons** is the paths of the node selection (nodes or a
  segment) and of the selected objects: the Node tool draws and hit-tests every path (`0006` adrs,
  "no editing set"), so "every open path of the editing set" needs a definition. With nothing
  selected the buttons are dimmed ("Select an open path with 3 or more nodes.").
- **`smooth_corner_handles` takes the tangent** (the vector from the node before to the node after),
  not the two points of the ADR's signature, so `convert_anchor_kind`, which already computes it for
  open ends, calls it unchanged.
- **Added ids are checked against the two paths of a growth, not the whole document:** the Pen mints
  them (`AnchorIdMinter`), so a collision cannot arise in a session; a document-wide scan on every
  Pen commit was not worth its cost.

## Validation

- Unit and session tests as above; `cargo nextest` for the whole workspace.
- Browser check: continue from an end (cursor, chip, new nodes, finish), close with the dashed
  preview and the chip, the Close path buttons and their notice.
