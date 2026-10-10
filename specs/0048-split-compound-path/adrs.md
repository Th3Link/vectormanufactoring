# ADRs for "Split: separate the unconnected parts of a compound path"

Split is the region plan of 0035 under a new name, extended by one rule
(regions whose outlines touch form one part). The new Break apart copies
every outline into an object of its own. Neither command does any geometry
beyond the touch and nesting tests that already exist. **No new dependency,
no new crate, no new ADR, no `format_version` bump**, no trait, no generic.
The one structural change: the replace command of 0035 becomes the shared
write for Split, Break apart, Cut and Fracture/Flatten.

## Depends on

- [ADR 0002 §5, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  pieces are new tree nodes at the compound path's place, written in one
  commit with every id resolved first.
- [ADR 0003 §1](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  the touch and nesting tests stay in `curvyo-geometry-core`.
- [`specs/0035-combine-and-break-apart/adrs.md`](../0035-combine-and-break-apart/adrs.md):
  `outline_touch::touching_outlines` (guarantee: pairs within 0.001 mm are
  always reported), `outline_nesting`, `plan_break_apart` (regions), the
  replace command, the shared refusal marks, the `boolean-budgets` CI job
  and the generated performance inputs.
- [`specs/0016-boolean-operations/adrs.md`](../0016-boolean-operations/adrs.md):
  compound path, nonzero fill, canonical winding, refusal pattern.

## Feature-local decisions

- **2026-10-10: Split = today's plan plus the touch rule, in
  `curvyo-ui-core/src/split.rs`.** `break_apart.rs` is renamed to
  `split.rs`, and `plan_break_apart` and `BreakApartRefusal` become
  `plan_split` and `SplitRefusal`. After the regions are formed, regions
  whose outlines appear in a `touching_outlines` pair (crossing included)
  are merged with a union-find over region indices, so each set is one part
  (criterion 4). Parts are ordered by their first outline (criterion 3). A
  compound path of one part is not rewritten (criterion 7). There is no
  geometry and no flattening of the result, so criterion 16 (1e-9 mm) holds
  by construction. The touch test flattens only to decide.
- **2026-10-10: the new Break apart, a new `curvyo-ui-core/src/break_apart.rs`.**
  `plan_break_apart` gives one piece per outline in the compound path's
  outline order, nodes copied as they are and **not reversed** (criterion
  11). A released hole keeps its negative winding. Nothing depends on the
  winding of a single-outline path: it paints the same under nonzero, and
  Combine reverses by depth. The notice flag "holes became filled shapes"
  is "any outline of odd depth" from `outline_nesting`. Both plans share
  one availability function (criterion 1).
- **2026-10-10: one replace command for every piece-making command.** In
  `curvyo-document-core/src/replace.rs`, `break_apart(parts)` becomes
  `replace_with_pieces(removed: &[NodeId], pieces: &[(NodeId,
  Vec<(Vec<NewAnchor>, bool)>)], label: &str) -> Result<Vec<NodeId>,
  ObjectEditError>`. Each piece names the object whose place and complete
  style it takes. Pieces of one object sit consecutively after it, in the
  given order. Every id in `removed` is deleted. Every id is resolved and
  every anchor id checked before the first write. Split passes the label
  `split` and Break apart `break_apart`. Later slices extend the same
  function and do not add another: Cut adds a fill choice (0036) and
  Fracture/Flatten pass objects in `removed` that get no piece (0037). This
  reverses the 0035 note that rejected a general replace "with one caller".
  It now has five callers.
- **2026-10-10: labels and golden files.** Labels `split` and `break_apart`
  are pinned by a test. Note that `break_apart` now names the every-outline
  command: a file saved after 0035 holds history steps labelled
  `break_apart` that did what Split does now. 0020's name table
  (`history_names.rs`) will name those old steps "Break apart". That is
  cosmetic, and the format is unaffected. The golden file `combine_ring_island.curvyo` of 0035 is
  reused: Split gives two objects, Break apart three. New golden file:
  `compound_overlapping_squares.curvyo` (criterion 4, a compound path that
  only another program writes).
- **2026-10-10: session and frontend.** `session/combine.rs` keeps Combine.
  Split and Break apart get `session/split.rs` (both commands, one shared
  availability) and the matching `wasm_split.rs`. The rail buttons are
  placed per the UX notes, in the Path card order Combine, Break apart,
  Split, Cut.
- **2026-10-10: budget (criterion 15).** Same job and generator pattern as
  0035. The part search on input (a) has its own budget of 500 ms. The
  union-find adds less than linear cost over `touching_outlines`.

## Flagged

- Spec Question 1 (rename 0035's command to Split, Break apart becomes the
  every-outline command) is the customer's. Default A, as written here. On
  B this spec is dropped and nothing above is built.
