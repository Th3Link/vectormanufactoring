# ADRs for "Path offset: Outset and Inset by a typed distance"

Offset runs on `i_overlay` `=9.0.1`'s own integer offsetter, on the same
snapped grid as the Boolean operations. **No new dependency, no `Cargo.toml`
change, no new crate, no `format_version` bump** (results are ordinary and
compound paths), no trait, no generic. The customer chose the library on
2026-10-10. It is recorded as a dated amendment to ADR 0003 §4.

## Depends on

- [ADR 0003 §1, §3, §4, §8](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  the kernel lives in `curvyo-geometry-core`, every operation takes an
  explicit `Tolerance`, results are polylines, the integer grid is 0.001 mm
  and `i_overlay` runs in its `i64` engine for determinism. There is no
  backend trait. The §4 amendment of 2026-10-10 (this slice) replaces "kurbo
  stroke expansion plus union" with `i_overlay`'s offsetter.
- [ADR 0002 §5, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  results are new tree nodes after their operands, written in one commit.
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): the
  preview and the entry's remembered values are ephemeral session state,
  never document state.
- [`specs/0016-boolean-operations/adrs.md`](../0016-boolean-operations/adrs.md):
  `Outline`, `grid_paths`, `normalize` (nonzero region, criterion 9),
  `cleanup`, `canonical_millimetres` (criteria 11, 12), `MAX_COORDINATE_MM`,
  `outline_of_rotated` for primitives, refusal codes with offenders and the
  refusal outline layer.

## Feature-local decisions

- **2026-10-10: use the integer API, not the float adapters.**
  `i_overlay::mesh::int::outline::offset::IntOutlineOffset` for closed
  operands and `mesh::int::stroke::offset::IntStrokeOffset` for open paths,
  on `i64`, with `MathMode::Integer`. The float traits `OutlineOffset` and
  `StrokeOffset` pick their scale from the input bounds. That makes the grid
  depend on the input, which the ADR 0003 §3 amendment forbids for
  criterion 12. The distance is rounded to the 0.001 mm grid before the call.
- **2026-10-10: the pipeline, one new module `curvyo-geometry-core/src/offset.rs`.**
  `offset(operand: &[Outline], distance_mm: f64, join: OffsetJoin, cap:
  OffsetCap, tolerance: Tolerance) -> Result<BooleanResult, OffsetError>`,
  called once per operand (criterion 10: no merging).
  - **Closed:** `grid_paths`, then `normalize`, then `outline(offset)`, then
    `cleanup`, then `canonical_millimetres`. `normalize`'s output is fed
    directly. It already has the winding `i_overlay` expects (outer positive,
    holes negative), and holes shrink when the outline grows (criterion 6).
  - **Open:** flatten the path, snap it, call `stroke(width = 2 x |d|, closed
    = false)`, then `cleanup` and `canonical_millimetres`. A negative `d` on
    an open path is refused before the kernel runs (criterion 8).
  - Flattening uses `tolerance - 2 x GRID`, as `boolean`. Round joins and
    caps get `ArcOptions { max_step = 2 x acos(1 - 0.01 mm / |d|) }`, clamped
    by the library to 0.35° to 45°. That keeps every chord within 0.01 mm
    and meets the node budget of criterion 11.
  - `validate_outline` and `validate_stroke` run first. An `Err` becomes the
    out-of-range refusal (criterion 14). An empty result becomes
    `OffsetError::Empty` (criterion 7). The stroke has radius
    `ceil(width / 2)` and gives nothing at one grid unit, so an open path
    offset by 0.001 mm is "empty" as well.
  - Joins: Round maps to `IntLineJoin::Round`, Bevel to `Bevel`, Miter to
    `Miter(2 x asin(1/4))` (28.955°, the SVG limit 4). Caps: Round, Flat
    (`Butt`), Square.
- **2026-10-10: near-straight miter cutoff.** The library closes miter turns
  below `miter_min_turn` (default 5°) with a straight edge. At large `|d|` on
  a flattened curve that edge misses the miter by up to `|d| x θ² / 8`
  (0.05 mm at 50 mm and 5°). Set `miter_min_turn = min(5°, sqrt(0.04 mm /
  |d|) rad)`, which keeps that error at 0.005 mm or less. Bevel and Round
  are unaffected.
- **2026-10-10: the document write.** It needs a new command in
  `curvyo-document-core/src/replace.rs`: `add_paths_above(pieces: &[(NodeId,
  Vec<(Vec<NewAnchor>, bool)>)], label) -> Result<Vec<NodeId>,
  ObjectEditError>`. It places each result directly above its operand with
  a copy of the operand's complete style and rotation 0, deletes nothing,
  and makes one commit labelled `offset_path` (criteria 10, 13). It reuses
  `create_path_after` and the anchor-id check.
- **2026-10-10: crate placement.** `curvyo-ui-core`: `offset_entry.rs` (entry
  state, ranges, the session-remembered values, availability and refusals,
  the plan with fresh anchor ids). `curvyo-render-core`: the blue hollow
  preview outlines, using the existing preview stroke and no new layer kind
  if the 0016 refusal layer's draw path fits. `curvyo-editor-wasm`:
  `session/offset.rs` and `wasm_offset.rs`, in the pattern of
  `session/combine.rs`. The preview is computed on each Distance, Join or
  Cap change and coalesced to one per animation frame. The last preview's
  result is cached, and Apply writes that result (criterion 3). The entry is
  the first step of the Escape cascade.
- **2026-10-10: CNC reuse.** The kernel function is the routine that CNC
  tool compensation and V-carving will call later (spec "Out of scope").
  Nothing CNC-specific is added now.
- **2026-10-10: tests.** Criteria 15 to 20 are unit tests in `offset.rs`.
  Criterion 12 joins the cross-target determinism check of 0016. A
  property test checks random grid polygons: the outset contains the
  operand, the inset lies inside it, and the area is monotonic in `d`.
  Budget (criterion 3): 200 nodes in under 50 ms, release build, in the
  `boolean-budgets` CI job.

## Flagged to the PO (criterion changes the spec invites)

1. **Criterion 19 and "Miter limit": `i_overlay` clips the miter, it does
   not bevel it.** Past the limit, it cuts the tip at the limit distance
   (4 x |d| from the corner, SVG 2 `miter-clip`). It does not cut at the
   offset edges' ends (|d| from the corner, SVG 1.1 `miter`). For the
   20-degree corner the two nodes lie 20 mm from the corner, not 5 mm. "No
   node 28.8 mm away" still holds. Default: take the library's clipped miter
   and reword criterion 19 and the definition. A true bevel past the limit
   would mean writing our own join, which is what the customer's library
   choice avoids.
2. **Criterion 11's "and so within 0.02 mm of the original curves" holds for
   Round only.** With Miter or Bevel, the joins at the chord corners of a
   flattened curve add about `0.01 mm x |d| / R` (R is the curve radius).
   The binding check, 0.01 mm to the offset of the flattened operand, holds
   for all joins. Default: the parenthesis applies to Round. The PO rewords
   it.
