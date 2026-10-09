# Plan for Segment drag bending (and the path-tools slice)

One branch, `story/path-tools`, one PR (customer rule of 2026-10-09: one PR per slice, opened
when the slice can be tried end to end and the full local gate is green). The slice is built in
milestones, in this order; M3 and M4 are added when their specs (`0034-pen-path-extension`,
`0035-combine-and-break-apart`) are Ready.

- **M1: the Boolean toolbox** (amendment of `0016-boolean-operations`, 2026-10-10).
- **M2: segment drag bending** (this story, `specification.md`, `adrs.md`).
- M3, M4: not planned yet.

`origin/main` is merged into the branch at every milestone; another implementer works on the
style panel (`0017`, `0018`, `0030`) and touches `session/draw.rs`, `session/mod.rs`, the `lib.rs`
export lists and `path_codec.rs`, so edits to those files here stay small and local.

## M1: the Boolean toolbox

Spec: `specs/0016-boolean-operations/specification.md`, "Amendment 2026-10-10"; design-system rows
"Boolean toolbox", "Tool rail architecture", "Toolbox card", "Rail keyboard model", "Rail tooltip
and notice placement", "Action notice". Frontend only (no Rust, no format change).

- [x] The five Boolean buttons become their own card below the tools card: the same surface as the
  tools card (`railCard.ts`), 8 px gap, no divider, no heading, one column, 224 px high (0016
  criteria 1, 2).
- [x] 4 px clear space under the rail at 800 x 600; in a viewport too short the tools card stays
  fixed and the cards below scroll inside the rail (criteria 1, 1b).
- [x] The notice is a child of the rail root, 12 px right of the rail's right edge
  (`--rail-right` + 12) and level with Union (criteria 15, 47a); tooltips stay right of the buttons.
- [x] `--rail-right` (60 px) and `--rail-inset` (12 px) tokens in `index.css`; the bars' overlay
  row uses `--rail-right` + 12.
- [x] Stale comments naming the "Boolean section" are updated.
- Validation: tsc, oxlint, `npm test`, a browser measurement of the rail at 800 x 600 (card ends 4
  px above the viewport bottom; notice 12 px right of the card).

## M2: segment drag bending

Crates: `curvyo-geometry-core` (shape rule), `curvyo-document-core` (command, closing segment of a
two-node closed path), `curvyo-ui-core` (the drag, hover test), `curvyo-render-core` (hover band,
handles of the end nodes), `curvyo-editor-wasm` (session routing, blue preview, readout, badge).
No new crate, no new dependency, no format change (`adrs.md`).

- [x] 1. `geometry-core`: `bend_segment_handles` and `BentHandles`; table and property tests
  (criteria 7 to 11). `tests/segment_bend.rs`.
- [x] 2. `document-core`: `segment_bend.rs` with `Document::bend_segment` (label `bend_segment`,
  one commit, atomic); the directed `adjacent_segment_indices` (criteria 6a, 12, 18, 19).
  `tests/segment_bend.rs`.
- [x] 3. `ui-core`: `segment_bend.rs` (`SegmentBend`, `BendResolution`, `segment_is_bendable`),
  `Drag::Bend` and `LiveNodeDrag::Bend` in the Node tool, `HitTolerances::drag_threshold`,
  `segment_pairs` for two-node closed paths, bounding-box reject in the segment hit test, the Select
  tool's axis lock shared (`lock_axis`) (criteria 1 to 6a, 13, 19, 20).
- [x] 4. `render-core`: `Hovered::Segment`, the 4 px `--segment-hover` band (`segment_band`),
  `DecorationInput::handle_nodes` (criteria 15, 16).
- [x] 5. `editor-wasm`: pointer routing, the blue preview over the committed artwork, the
  selected-segment overlay suppressed while the bend runs, the "Δ x, y mm" readout and the Lock
  badge, no hover test during a drag, one document read per Node-tool drag (criteria 14 to 17, 20).
  `tests/segment_bend.rs`.
- [x] 6. Frontend: the Node tool's rail tooltip second line (criterion 23). Pointer capture and
  the rest of the gesture were checked in the browser: a drag on a segment bends it and leaves it
  selected; the band flanks, computed from the token values, are 1.86:1 on `--canvas-bg`, 1.53:1 on
  `--pasteboard-bg` and 2.02:1 over a white fill (all at least 1.5:1, so 50 % stays).
- [x] 7. Benchmark budgets as an `#[ignore]` test (`curvyo-editor-wasm/tests/segment_bend_budget.rs`,
  5000 nodes, release): bend frame at most 7.6 ms after the first frame (which reads the document
  once: 19 ms), one hover hit test about 0.1 ms; budgets 12 ms and 2 ms (criterion 17).

## Validation

- Unit and property tests per crate as above; session-level acceptance tests in
  `curvyo-editor-wasm/tests/segment_bend.rs`.
- Browser check of the hover band, the bend and Escape at 100 % and 800 % zoom, and the flank
  contrast of the band (`--segment-hover` on `--canvas-bg`, `--pasteboard-bg` and over a fill).
- Gate: `CLAUDE.md` §7 in full, run locally before the PR is opened.
