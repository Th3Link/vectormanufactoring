# ADRs for "Fracture and Flatten"

The spec asked whether to build both commands from the pairwise operations of
0016 or from a planar-arrangement routine. **Decision: incremental boolean
folds on the kernel's integer grid, one new module per command in
`curvyo-geometry-core`.** There is no arrangement routine. **No new
dependency, no new crate, no new ADR, no `format_version` bump** (pieces are
ordinary and compound paths), no trait, no generic.

## Depends on

- [ADR 0003 §1, §3, §8](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  flattened polygons, `i_overlay` `=9.0.1` in its `i64` engine on the
  0.001 mm grid, polyline results, no backend trait.
- [ADR 0002 §5, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  pieces take their owner's place in sibling order, written in one commit.
- [`specs/0016-boolean-operations/adrs.md`](../0016-boolean-operations/adrs.md):
  the pipeline (`grid_paths`, `normalize`, `combine`, `cleanup`,
  `canonical_millimetres`), the region under nonzero regardless of paint,
  refusal codes with offenders, determinism (criterion 16), and the fixtures
  (a) to (i) that criterion 17(c) reuses.
- [`specs/0035-combine-and-break-apart/adrs.md`](../0035-combine-and-break-apart/adrs.md):
  `outline_nesting` (a piece is an outer outline with the holes directly
  inside it), `session/refusal.rs`, the `boolean-budgets` CI job.
- [`specs/0048-split-compound-path/adrs.md`](../0048-split-compound-path/adrs.md):
  `replace_with_pieces`, the shared write.

## Feature-local decisions

- **2026-10-10: folds on the grid, not an arrangement.** Operands are taken
  in stacking order from the top, as grid regions (`normalize` of
  `grid_paths`).
  - **Flatten** (`visible_region.rs`): keep `U`, the union of the operands
    already seen. Each operand's visible region is `O − U`, then `U := U ∪
    O`. That is 2n two-operand overlays.
  - **Fracture** (`fracture.rs`): keep a list of cells, each a covering set
    plus a grid region. A new operand O splits every cell whose box meets
    O's box into `c ∩ O` and `c − O`, and adds `O − ∪cells` as a cell of its
    own. The owner is the topmost member of the covering set.

  Both run on `Paths64` with the crate-private `normalize`, `combine` and
  `run` of `boolean_grid.rs`. `cleanup` and `canonical_millimetres` run
  **once per output piece at the end, never between steps**. Cleanup removes
  near-collinear nodes, so running it at every step would let up to 2000
  steps drift. Each result is cut into pieces with `outline_nesting`
  (criterion 7: a cell is connected). Rejected: a planar arrangement (a
  DCEL over all edges, then face classification). It is the textbook way,
  but it is a new robustness surface we would own, next to an integer
  engine that has already passed the property tests of 0016. The fold costs
  about cells x overlapping operands, and box filtering keeps fixture (b)
  (2,000 squares in a chain) at about 6,000 small overlays.
- **2026-10-10: names.** The kernel already has `flatten` (curve to
  polyline). The command's kernel module is therefore `visible_region.rs`
  (`visible_regions`), and the `ui-core` module is `flatten_command.rs`. A
  reader must never mistake one for the other. Labels `fracture` and
  `flatten`.
- **2026-10-10: the kernel results.** `fracture_cells(operands) ->
  Result<Fracture { pieces: Vec<(owner index, outlines)>, untouched: Vec<index>
  }, StackError>` and `visible_regions(operands) -> Result<Vec<Visible>,
  StackError>`, where `Visible` is `Untouched`, `Trimmed(outlines)` or
  `Hidden`. "Untouched" means the operand's grid region meets no other
  operand's in area (criteria 6, 10). Such an operand is passed through
  unchanged and keeps its curves. "No two operands overlap" (criterion 3)
  is "every operand untouched". Touching edges give an empty intersection
  on the grid, so they do not count. Pieces of one owner are sorted by
  their first node (x, then y; `canonical_millimetres` starts each outline
  at its smallest vertex), as criterion 8 asks.
- **2026-10-10: the write is `replace_with_pieces` from 0048, with `removed ⊋
  owners`.** Fracture removes every participating operand, including one
  that owns no cell because it is fully covered. Flatten removes the
  trimmed and the hidden operands. Pieces take their owner's place and
  complete style. 0037 adds only the case "removed object with no piece".
  The 5,000-piece limit (criterion 4) is checked before the write.
- **2026-10-10: crate placement.** `curvyo-geometry-core`: `fracture.rs`,
  `visible_region.rs`, and widened visibility of `combine` in
  `boolean.rs`, crate-private. `curvyo-ui-core`: `fracture.rs` and
  `flatten_command.rs` (availability, refusals, plans with fresh anchor
  ids). `curvyo-editor-wasm`: `session/fracture.rs` (both commands) and
  `wasm_fracture.rs`. `frontend`: two rail buttons per the UX notes.
- **2026-10-10: tests and budgets.** The examples of criteria 7, 10 and 11
  are unit tests. Criterion 17: (a) and (b) come from a deterministic
  generator in the test, and the expected counts are golden. (c) reuses the
  0016 fixture files. Each runs in under 2 s in a release build in
  `boolean-budgets`. Property test: random grid squares, where the sum of
  the piece areas equals the area of the union, no two pieces overlap, and
  the visible regions of Flatten tile the union.

## Flagged to the PO (no customer question)

1. **Criterion 7 says "the cells' set of covering operands differs from
   piece to piece".** Under the spec's own definition (a cell is
   connected), two separate areas with the same covering set are two
   pieces. Example: a bar laid across both arms of a U overlaps it in two
   separate places. Default: the
   definition holds, and the sentence means adjacent pieces. The PO
   rewords it.
