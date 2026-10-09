# ADRs for "Combine and Break apart"

Combine and Break apart only regroup existing outlines into objects. They
reuse 0016's compound path and its one replace command, and add two decision
functions to the geometry kernel: whether outlines touch, and how they nest.
**No new crate, no new dependency, no new ADR, no `format_version` bump, no
trait, no generic.** Reference state: `main` at `909a2cc` (0016 merged).

## Depends on

- [ADR 0002 §5, §6, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  z-order is sibling order. Paths hold several closed subpaths. Each command
  is one commit, with every id resolved before the first write.
- [ADR 0003 §1, §3](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  anything that has to know what a Bézier is lives in `curvyo-geometry-core`
  and takes an explicit tolerance. No backend trait.
- [ADR 0001 §1, §3](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  availability and plans are pure `ui-core` functions; the frontend renders
  codes.
- [`specs/0016-boolean-operations/adrs.md`](../0016-boolean-operations/adrs.md):
  the compound path (`extra_subpaths`, nonzero fill, anchor ids unique per
  document), `replace_with_path` (result after the base, base style, rotation
  0, operands deleted, one commit), canonical winding (shapes have positive
  shoelace area in document coordinates, holes negative), the kernel's
  deterministic `flatten`, `contains_point_in_outlines`, `signed_area_mm2`,
  `MAX_COORDINATE_MM`, commands that are not tools, refusal codes with
  offenders, the refusal outline layer, and `boolean_availability` as the
  pattern.
- [`specs/0034-pen-path-extension/adrs.md`](../0034-pen-path-extension/adrs.md):
  the shared `reversed_anchors` helper (whichever slice merges first adds it).

## Feature-local decisions

- **2026-10-10: Combine writes through `replace_with_path`, unchanged.**
  The call is `replace_with_path(operands, bottom_most, outlines,
  "combine_paths")`. That covers criteria 3, 6 and 7: the result's place,
  the bottom-most operand's whole style, rotation 0, one commit. Outlines are
  passed in stacking order from the bottom, and in each operand's own order.
  Each outline whose winding disagrees with its depth goes through
  `reversed_anchors` (criterion 4). No other arithmetic happens, so
  criterion 8 holds by construction. Primitives contribute
  `outline_of_rotated`, the same outline that Object to path writes, with
  its kinds (criterion 5). Anchor ids are minted fresh with `AnchorIdMinter`,
  as for booleans and as criterion 14 asks for Break apart.
- **2026-10-10: Break apart is one new command in the same module,
  `curvyo-document-core/src/replace.rs`.** Its private core, which resolves,
  checks ids, creates a node after the base, writes the base style and
  deletes the operands, is extracted uncommitted. `replace_with_path` keeps
  its signature and calls it.

  ```rust
  pub fn break_apart(&self, parts: &[(NodeId, Vec<Vec<(Vec<NewAnchor>, bool)>>)])
      -> Result<Vec<NodeId>, ObjectEditError>        // label "break_apart"
  ```

  For each compound path it creates one object per region, consecutively at
  the compound's place and each with the compound's whole style. It deletes
  the compound. Every compound in the selection is written in one commit
  (criteria 13 to 15, 17). A region with one outline is written as an
  ordinary closed path, because a path with one outline already is one.
  Rejected: N calls to `replace_with_path` (N commits; breaks criterion 17),
  and a general `replace_objects(&[Replacement])` (a second public shape
  with one caller).
- **2026-10-10: two new kernel functions in `curvyo-geometry-core`, one
  module each.**
  - `outline_touch.rs`: `touching_outlines(outlines: &[Outline]) ->
    Vec<(usize, usize)>`. It flattens every outline with the kernel's own
    `flatten` at 0.0005 mm. It sweeps all chords sorted by their minimum x,
    keeping an active list, and tests the pairs whose boxes come within
    0.002 mm. A pair is reported when its chords are 0.002 mm apart or
    closer. Adjacent chords of one outline are skipped, so a self-crossing
    outline reports `(i, i)`. **Guarantee:** every pair whose true curves
    are within 0.001 mm (crossing included) is reported, and no pair 0.003
    mm or more apart is reported. Each polyline lies within 0.0005 mm of its
    curve, so the triangle inequality gives both bounds. Lines are exact:
    two squares 0.01 mm apart pass, and a shared edge is refused
    (criterion 10). The constants are named (`TOUCH_DISTANCE` 0.001 mm,
    `TOUCH_FLATTEN` 0.0005 mm) and are not settings. Rejected: running
    `i_overlay` to find touches. An area comparison misses edges that are
    shared without overlap, and the grid (0.001 mm) is as coarse as the
    threshold.
  - `outline_nesting.rs`: `outline_nesting(outlines) -> Vec<Nesting { depth,
    parent: Option<usize> }>`. An outline is enclosed by another when its
    test point lies inside the other by `contains_point_in_outlines` (one
    outline, nonzero) and the test point lies in the other's bounding box.
    The test point is the first node. If that node lies within
    `TOUCH_DISTANCE` of the other outline, the next node is used (boolean
    results may touch at a vertex); an outline whose nodes all touch counts
    as not enclosed. `parent` is the enclosing outline of depth `depth − 1`.
    The bounding-box reject makes nested and side-by-side inputs cheap.
    Kurbo's winding uses root finding, so it can round differently on
    different platforms. That rounding cannot change a decision, because no
    test point is within 0.001 mm of the outline it is tested against.
  - The winding of an outline is the sign of `signed_area_mm2` of its
    flattened polyline. An |area| at or below 1e-6 mm² (one grid cell) is
    "encloses no area" (criterion 11, as `0016` criterion 16).
- **2026-10-10: planning in a new module `curvyo-ui-core/src/combine.rs`.**
  `boolean.rs` is at 482 lines. Its operand helpers (operands in z-order,
  outlines from snapshots, the open check, the range check) become
  `pub(crate)` and are called from `combine.rs`, not copied.
  - `combine_availability(objects, selection) -> CombineAvailability {
    combine: bool, break_apart: bool }`. Combine needs at least two selected
    objects, Break apart at least one. The session passes an empty selection
    outside the Select tool, as for booleans (criterion 1).
  - `plan_combine(objects, selection, minter) -> Result<CombinePlan,
    CombineRefusal>`. Checks run in this order and list every offender:
    groups (a variant added by `0023` when groups exist; nothing to check
    before that), `OpenPaths`, `OutOfRange` (`MAX_COORDINATE_MM`), `NoArea`,
    then `Touching { offenders, of }`. The offenders of a touching pair are
    the operands that own the two outlines. A self-crossing operand counts
    as touching (flag 1).
  - `plan_break_apart(objects, selection, minter) -> Result<BreakApartPlan,
    BreakApartRefusal>`. It calls `outline_nesting` on each compound path's
    own outlines. Each region is an even-depth outline plus its children at
    `depth + 1`. Regions are ordered by their first outline. Nodes are
    copied without reversal (criterion 14). The selection afterwards is the
    pieces plus the untouched non-compound objects (criterion 15). Refusal:
    `NoCompound` (criterion 16).
- **2026-10-10: session and frontend.** `editor-wasm` gets
  `session/combine.rs` and `wasm_combine.rs`, built like `session/boolean.rs`
  and `wasm_boolean.rs`: Select tool only, no drag in flight, the outcome as
  a code. The refusal outline reuses 0016's `RefusalMarks` and draw layer
  (criteria 9 to 12). `RefusalMarks` moves out of `session/boolean.rs` into
  `session/refusal.rs`, and the `Session` field `boolean_refusal` becomes
  `command_refusal`. It is one mark for any rail command, with no second
  copy. Notice and refusal sentences live in the frontend, keyed by code
  (`booleanText.ts` pattern). **Placement in the rail follows the UX notes**
  (the rail architecture the ux-engineer is designing now). The Rust side
  does not depend on where the buttons sit. Commands are not tools: the
  `Tool` enum does not grow, and there are no shortcuts.
- **2026-10-10: format impact: none beyond 0016's version 8.** A combined
  path stores curved anchors in `extra_subpaths`, which the schema already
  allows (same anchor maps; booleans happen to write straight ones). The new
  labels `combine_paths` and `break_apart` are pinned by a test. Golden file:
  `combine_ring_island.curvyo` (the ring with an island from criterion 13,
  saved after Combine) round-trips and breaks apart into the expected two
  objects (criterion 18).
- **2026-10-10: performance budget (criterion 19), release build, asserted in
  the `boolean-budgets` CI job.** Each of the four inputs runs a full
  Combine or Break apart through `Session` in under 2 s. The kernel part
  (touch plus nesting) has its own budget of 500 ms on input (a), the
  largest chord count (about 150 000 chords at 0.0005 mm). Expected cost:
  the sweep is near-linear for disjoint shapes; nested squares (c) make
  about 125 000 box pairs of 16 chord tests each; the document write for
  (d) is 5000 new objects (0016 measured 37 ms for 4000 anchors in one
  object, so this is a few hundred ms). The four inputs are built by a
  deterministic generator in the test, not stored as binary fixtures
  (flag 2). The expected counts are the golden values.

## Files shared with other slices

- **0017**: the `ui-core` and `document-core` `lib.rs` export lists and
  `editor-wasm/src/session/mod.rs` (the refusal field). 0035 does not touch
  the style codec, the `render-core` gradient code, the `ui-core` style
  modules, `session/style*.rs` or the panel.
- **0031**: the `geometry-core/src/lib.rs` export list (0031 adds
  `bend_segment_handles` in `segment.rs`; 0035 adds two new modules and does
  not touch `segment.rs`), the `ui-core` `lib.rs`, and `session/mod.rs`.
  These are rebase-level conflicts. 0035 does not touch `segment_bend`,
  `node_tool.rs`, `render-core` node overlays or `session/node.rs`.
- **0034**: `path_model.rs` (`reversed_anchors`), the export lists and
  `session/mod.rs`. They are not run in parallel (`CLAUDE.md` §4, same
  crates); 0034 goes first. 0036 to 0038 share the rail component and
  `geometry-core`.

## Milestones on one branch (`story/combine-and-break-apart`)

1. `geometry-core`: `outline_touch.rs`, `outline_nesting.rs`, unit and
   property tests (touch bounds, nesting of random disjoint discs), the
   kernel budget (criteria 4, 10, 19 at kernel level).
2. `document-core`: `break_apart` and the extracted core in `replace.rs`,
   `reversed_anchors` if 0034 has not merged, labels, refusal-writes-nothing
   tests (criteria 13 to 15, 17).
3. `ui-core` + `editor-wasm`: `combine.rs`, availability, plans, refusals,
   `session/combine.rs`, `wasm_combine.rs`, the shared refusal marks, the
   golden round trip and the session budgets (criteria 1, 3 to 18, 19).
4. Frontend per the UX notes (rail buttons, tooltips of criterion 2,
   notices). Then the UX review and the tester pass on the whole slice.

## Flagged to the PO (defaults taken)

1. **Self-crossing operand.** The spec only defines touches between
   outlines. Default: an outline that crosses or touches itself is refused
   with the touch message and counts as one touching object. The winding of
   a figure-eight has no meaning by depth.
2. **Criterion 19 says "fixtures in `tests/fixtures/`, golden files".**
   Default: the four inputs are generated in the test (a 5000-square
   `.curvyo` would be hundreds of KB in git), and the expected counts are
   the golden values. The criterion 18 round trip has a real golden file.
3. **Criterion 11, groups**: groups do not exist before `0023`. If 0035
   merges first, `0023` adds the group refusal (its criterion 28).
