# ADRs for "Boolean operations"

This slice adds the first real kernel operation and the first document-model
change since `stroke-and-fill-styling`: a path object may hold more than one
outline. **No new crate, no new ADR, one new external dependency
(`i_overlay`; the 2026-10-04 spike chose `clipper2-rust`, replaced on
2026-10-09, see "what PR 1 settled"), one `format_version`
bump.** The customer approved the compound-path decision and its file-format
change on 2026-10-09 (`CLAUDE.md` §3, document model; "Customer decisions"
below).

## Depends on

- [ADR 0001 §1, §3](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the rail buttons' enabled state and the command live in `curvyo-ui-core` as
  pure functions; `curvyo-editor-wasm` only binds; the frontend renders the
  state and forwards clicks.
- [ADR 0002 §6](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  "paths are sequences of cubic Bézier segments and lines, with explicit
  open/closed subpaths". The compound path below implements this accepted
  text; it does not amend it, so it needs no new ADR.
- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  sibling order is z-order. "Bottom-most" and "the base operand's place" are
  read from `Document::object_ids()` order, never from selection order (AC 9,
  22).
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one interaction, one commit. A boolean deletes the operands and creates the
  result in one commit; every id is resolved before the first write (AC 28).
- [ADR 0003 §1, §3, §8](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  the kernel lives in `curvyo-geometry-core`, takes an explicit `Tolerance`,
  runs on flattened polygons and returns polylines. The library is
  `i_overlay` (a candidate of the spike note under §3; it replaced the spike's
  pick `clipper2-rust` on 2026-10-09, see "what PR 1 settled"). No backend
  trait.
- [ADR 0003 §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  the 0.01 mm kernel tolerance is not the display tessellation tolerance.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  forces the `format_version` bump (an older reader would silently drop every
  outline but the first).
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md): every
  outline's anchors are a movable list of per-anchor maps, like today's.
  A peer's concurrent edit of an operand is lost when the operand is deleted
  by the boolean: the same delete floor as Delete.
- [ADR 0011 §3, §6](../../docs/adr/0011-workspace-and-crate-layout.md): crate
  edges unchanged; the new dependency must stay off the wasm32 ban list.
- [`specs/0006-path-merge-split-and-node-types/adrs.md`](../0006-path-merge-split-and-node-types/adrs.md):
  caller-minted `AnchorId`s, "a refusal writes nothing", the `format_version`
  merge rule (the merging PR takes `main`'s current version + 1).
- [`specs/0010-edit-interaction-polish/`](../0010-edit-interaction-polish/)
  criteria 54, 55: the key table and its gate; booleans add no entry.

## Feature-local decisions

### 2026-10-09: customer decisions (final)

1. **Compound path: option A**, one object with several outlines, encoded
   as `extra_subpaths` (next section). The document-model and file-format
   change is **approved by the customer**; the next section is accepted as
   written.
2. **Exclusion and Reverse difference are in:** five operations. No kernel
   change: the kernel keeps four operations (see "the kernel").
3. **The five operations are command buttons in their own group in the left
   tool rail**, under the existing tools, always visible, greyed out when
   fewer than two objects are selected. They are **no longer a Select bar
   group**. Consequences: "boolean commands in the tool rail" below.
4. **No keyboard shortcuts for now** (criterion 3 stands).

### 2026-10-09: compound path = option A, one object with several outlines (accepted by the customer 2026-10-09)

Answers the PO's open question 1. **Recommendation and default: A.**

- **(A) One path object holds one or more outlines. Chosen.** A ring is one
  object that fills as a ring, holes stay holes through transforms and later
  booleans, and it is what ADR 0002 §6 already promised. Cost: a format bump
  and an audit of every one-outline assumption (list below).
- **(B) Every result outline becomes its own path object.** No model change.
  Rejected: a filled plate minus a hole paints the hole over (two filled
  discs), the result of one boolean is several objects although AC 19 says
  one, and feeding those objects into the next boolean loses which outline
  was a hole. B is only correct for stroke-only work, and the format change A
  needs comes back with SVG import (multi-subpath `<path>`) anyway.

**Encoding (on disk and in the CRDT).** The existing keys stay the first
outline; one new optional key holds the others:

```text
meta map of a path (unchanged keys):
  closed, anchors, rotation, style keys      first outline, as today
  extra_subpaths : movable list, optional    NEW; absent = one-outline path
    each element a map:
      closed  : bool                         LWW register
      anchors : movable list of anchor maps  same schema as `anchors`
```

- Rejected: replacing `anchors` with a uniform `subpaths` list for every
  path. Old files are never rewritten on open (AC 37), so the reader and
  every writer would handle two shapes forever; `paths.rs`,
  `path_topology.rs` and `objects.rs` would all branch on it. The chosen
  shape leaves every existing path and every node-editing command untouched.
- Rejected: a compact packed-coordinate list for boolean outlines (fast,
  small). It would be a second path schema that the Node tool cannot edit,
  and criterion 20 makes a one-outline result an ordinary path. Revisit
  **only** if the PR 2 measurement below fails.
- Rejected: subpaths as child tree nodes. Children are reserved for groups
  (`layers-and-grouping`).
- Every anchor in every outline has a globally unique, caller-minted
  `AnchorId`. That lets a later node-editing slice (PO question 7, option B),
  Break Apart and Combine address nodes as `(NodeId, AnchorId)` with no
  outline index and **without another format change**.
- A writer never writes an empty `extra_subpaths`; a reader treats an empty
  list like an absent one. A boolean writes only closed outlines with at least
  3 anchors; the reader still accepts open ones (ADR 0002 §6, glyphs, SVG
  import later).
- **Fill rule:** nonzero over all outlines of the object together, fixed. No
  `fill_rule` register: no story needs even-odd. SVG import can turn an
  even-odd path into nonzero outlines with the kernel when it arrives.
- **Stroke:** every outline is stroked with the object's one style. Stroke
  markers (`stroke-markers`) need a rule for compound paths; flagged to that
  story, not decided here.
- **Open-file validation** (`validate_path_node`): a present
  `extra_subpaths` must be a movable list of maps, each with an `anchors`
  movable list whose elements pass `validate_anchor`. Anything else is
  `OpenError::Damaged`.
- **`format_version`:** `main`'s current value + 1 at merge (7 today; other
  drafts claim 8 and 9 provisionally; `document-size-and-rulers` takes none,
  its `adrs.md` decision 1). Migration from older versions: none, the key is
  simply absent. Golden fixtures: a compound path saved and reopened (AC 30,
  37) and a pre-bump file opened unchanged and not rewritten.

**Read model.** `PathSnapshot` keeps `closed` and `anchors` (the first
outline) and gains `extra_subpaths: Vec<SubpathSnapshot>` (`closed`,
`anchors`), empty for every one-outline path, plus `is_compound()` and a
`subpaths()` iterator over all outlines, first one included.
`document.json` skips the field when empty, so one-outline paths export as
before.

**The audit (the real cost of A).** Adding a field does not make the compiler
find the readers that ignore it. PR 2 changes each of these and adds one test
per line, mostly covered by AC 31 to 38:

| Where | What changes |
|---|---|
| `path_codec` (+ a new `subpath_codec` module; `path_codec` is at 474 lines) | read, write, validate `extra_subpaths` |
| `PathSnapshot::rotated`, `scaled`, `sheared` | map every outline |
| `objects.rs`: `translate_objects`, `duplicate_objects` (`renumber_anchors`, `check_anchor_ids`, `translate_path_meta`), `rotate_object`; `resize_path`; skew commit | write every outline; `CopySource::anchor_ids` counts all outlines' anchors in `subpaths()` order |
| `geometry-core::contains_point` | takes several outlines, one `BezPath`, winding summed (AC 33) |
| `ui-core`: `hit_test_object` (`outline_of`, `fills_point`, cheap reject), `object_bounds`, `oriented_box` | all outlines (AC 33, 34) |
| `render-core` artwork, stroke, dash | one lyon path with one subpath per outline; fill is already `FillRule::NonZero` (AC 31, 32) |
| Node tool (`Session::paths()`), `check_join`, `check_split` | skip or refuse compound paths; hint of AC 38 |

`docs/technical-debt.md` gets an entry at PR 2's merge: "one-outline
assumptions are found by audit, not by the compiler", with this table.

### 2026-10-09: the kernel

- **Library:** `i_overlay =9.0.1`, pinned exactly, `default-features = false`
  (the default feature set is empty; `allow_multithreading`, which pulls
  `rayon`, stays off). Licenses: `i_overlay` MIT OR Apache-2.0, `i_float`,
  `i_shape`, `i_key_sort`, `i_tree` MIT, plus the pure-Rust `libm`; `cargo deny`
  passes without a change. A workspace dependency used only by
  `curvyo-geometry-core`. The spike picked `clipper2-rust`; it was replaced
  during PR 1 (see "what PR 1 settled"). An update of the pin is a reviewed PR
  that reruns the fixtures and the property tests (command below), not a
  Renovate auto-bump.
- **Input and output.** The kernel takes operands as lists of outlines
  (`&[OutlineTriple]` plus `closed`, the triple `curvyo-ui-core` already
  builds for hit-testing) and returns `Vec<Vec<Point>>` in a `BooleanResult`.
  No `kurbo` or `i_overlay` type crosses the crate API. It does not take
  `PathSnapshot`, so PR 1 does not wait for PR 2.
- **Operations:** `Union`, `Difference` (first operand minus the rest),
  `Intersection`, `Exclusion`. Reverse difference is `Difference` with the
  caller putting the top-most operand first; no fifth kernel operation.
- **Pipeline**, in this order (the module doc of `boolean.rs` is the same
  list):
  1. Refuse, in this order and listing every offending operand: no operands,
     a tolerance not above two grid pitches, an open outline
     (`OpenOperands`, AC 15; the kernel enforces it, not only the UI), a
     non-finite or out-of-range coordinate or handle (`OutOfRange`).
  2. Flatten every segment with the kernel's own subdivision (not `kurbo`) to
     `tolerance − 2·GRID` (0.008 mm for the fixed 0.01 mm), leaving room for
     grid rounding and the cleanup. Disc of radius 10 mm: 84 nodes, inside
     AC 26's 71 to 142.
  3. Snap to an integer grid, `GRID` = 0.001 mm (a named constant inside the
     kernel, not a setting), and run on `Overlay<i64>`. The range check is
     |coordinate| ≤ 10⁷ mm, which is 10¹⁰ grid units, far inside the engine's
     ±2⁶² units; 100 000 mm is 10⁸ units (AC 42).
  4. **Normalize each operand on its own:** a nonzero overlay of the operand
     with nothing. This gives every operand positive orientation and its
     painted region (self-intersections, AC 7; holes of a compound operand,
     AC 8). Without it, two operands wound in opposite directions would
     cancel in their overlap under nonzero. An operand that comes out empty:
     `EmptyOperands` (AC 16).
  5. The operation: Union = one nonzero union of all; Difference = first
     operand as subject, the rest as clip; Intersection and Exclusion are
     **folded pairwise** (the subject/clip form computes `A ∩ (B ∪ C)`, not
     `A ∩ B ∩ C`, and its XOR is not odd-parity for three or more operands).
  6. Cleanup (`boolean_cleanup.rs`), in up to four rounds: remove vertices
     within one grid unit of the chord that replaces them, repair crossings
     with a nonzero union, drop outlines of fewer than 3 points, no area or an
     average width under one grid pitch (rounding noise). AC 24 and 41.
  7. Canonical form (AC 41, 43): outer outlines have positive shoelace area
     in document coordinates (clockwise on screen, Y down), holes negative;
     each outline starts at its smallest grid point (x, then y); outlines are
     sorted by that start point, ties by signed area, larger first. Then back
     to mm (integer × 0.001).
  8. Empty result: `EmptyResult` (AC 17).
- **Determinism (AC 43):** from the snap on, the work is integer arithmetic;
  flattening and the cleanup's comparisons use IEEE basic operations and
  `sqrt`, which agree on x86-64, aarch64 and wasm32; Rust does not contract
  them into fused multiply-adds. `kurbo`'s flattening was checked and calls
  `powf`, so the kernel flattens itself. `i_float` calls `libm` in its float
  adapter only; the kernel uses the integer engine. No `HashMap` iteration
  order anywhere in the kernel. Output lies on the grid, so the golden files
  agree exactly, not just to 1e-6 mm: CI compares them on Linux, macOS
  (aarch64) and Windows. The same results in the wasm build are argued, not
  tested (technical debt, for PR 3).
- **Robustness:** `catch_unwind` and timeouts are unavailable in a core
  crate (no threads, wasm aborts on panic), and `i_overlay` returns no errors,
  so the guard is the test suite: the 9 fixtures of AC 40 (also checked
  against the output rules of AC 41), fixed-seed property tests on random
  polygon pairs with holes and self-intersections, and a coarse lattice that
  provokes coincident and collinear edges. To repeat the runs that justified
  the library, for example after a pin update:
  `PROPTEST_CASES=20000 cargo test --release -p curvyo-geometry-core --test
  boolean_properties` (256 cases per property by default; `lattice_shapes_stay_valid`
  is the property that separated the two libraries).
- **Spec issue, AC 39 (for the PO):** grid snapping cannot guarantee "less
  than 0.0004 mm apart is coincident": two values 0.0004 mm apart can round
  to neighbouring grid points. What the kernel guarantees: coordinates that
  round to the same 0.001 mm grid point coincide, and edges 0.002 mm or more
  apart stay separate. Proposed wording: test the 0.0004 mm gap at
  grid-aligned positions (20.0000 and 20.0004). Rejected: a morphological
  close (offset +δ, −δ) to merge near-gaps; it rounds concave corners and is
  a second geometric policy nobody asked for.
- **Performance (AC 44 to 47):** realistic for the kernel. The spike did a
  5,000-subpath intersection in 9 to 14 ms; 2 × 1,000 curved nodes flatten
  to a few thousand vertices (low milliseconds), 2 × 10,000 to at most
  about 200,000 (well under 2 s). **The risk is
  the document write, not the kernel:** every result node is a Loro map with
  five registers and a 32-character id, about 100 bytes in the operation log.
  AC 46 (1,000 rectangles) and AC 47 (150 ms to repaint) are dominated by that
  write and by reading the snapshot back. PR 2 measures writing and reading a
  20,000-anchor compound path before it merges, because the encoding is the
  hard-to-change part. If it misses, the fallback is the packed encoding
  rejected above, as a new decision here and not a silent change.


### 2026-10-09: what PR 1 settled (implementer)

- **The library was replaced during PR 1.** The spike's pick, `clipper2-rust`
  1.2.0, returned wrong unions on reproducing inputs (a triangle of 0.4 to
  0.5 mm² filled that no operand covers; regression tests
  `a_union_with_collinear_overlapping_edges_is_exact` and
  `a_union_with_a_vertex_beside_a_long_edge_is_exact`), and its
  `SimplifyPaths` made a valid outline cross itself. Comparison on random pairs
  of 1 to 3 outlines of 3 to 8 vertices, area identities of criterion 14:
  - 6 mm lattice (many shared and collinear edges): `clipper2-rust` broke the
    identities in 12 to 16 of 5,877 pairs; `i_overlay` 9.0.0 (a harness
    outside the repository, same pairs) in none; `i_overlay` 9.0.1 inside the
    kernel in none of 19,643.
  - Random, up to 60 mm: none for either library in 6,000 to 12,000 pairs;
    none for the kernel on `i_overlay` in 19,999. A 20,000-pair run had found
    one miss with `clipper2-rust`.
  - Random, up to 600 mm: none for either, 6,000 pairs.

  The lead decided on 2026-10-09 to replace it (the customer may veto). The
  swap touched `boolean_grid::run` and the pipeline's types, not the API, the
  result type or the fixtures, except that the golden file of the bow-tie
  exclusion changed (two triangles touching at the pinch instead of one pinched
  outline, both valid). `i_overlay` contains `unsafe`: 49 sites in
  `i_overlay` 9.0.0's own source, about 130 in the five `i_*` crates of this
  lockfile (`i_overlay`, `i_float`, `i_shape`, `i_key_sort`, `i_tree`); our crate
  stays `forbid(unsafe_code)`. The 9.0.0 figures above are evidence only; the
  pin, the 19,643 + 19,999 kernel pairs, the goldens and the budgets are
  9.0.1.
- **Flattening is the kernel's own** (`flatten.rs`): `kurbo`'s `flatten` calls
  `powf` (in `to_quads`), which platform math libraries may round differently,
  so criterion 43 would not hold. The step count is `ceil(sqrt(M / (8 tol)))`
  with `M = 6 max(|P2 - 2 P1 + P0|, |P3 - 2 P2 + P1|)`, from the chord error
  bound `h² max|B''| / 8`; a segment whose inner control points are within the
  tolerance of its chord is one step. Only `+ - * /` and `sqrt`.
- **Errors carry every offending operand**, not one: `OpenOperands(Vec<usize>)`,
  `OutOfRange(Vec<usize>)`, `EmptyOperands(Vec<usize>)`, because criteria 15 and
  16 need the count ("1 of 3 selected objects") and the red outline of every
  offender. Further errors: `NoOperands`, `ToleranceTooSmall` (the tolerance
  must exceed two grid pitches) and `EmptyResult`.
- **Cleanup is the kernel's own** (`boolean_cleanup.rs`) and drift-free. A first
  version judged each vertex against the neighbours that remained; on a
  polyline circle of 100,000 vertices it collapsed the outline to 24 nodes up
  to 0.42 mm off its input. Now Douglas-Peucker keeps every input vertex within
  one grid unit of the result, and a second step removes kept vertices that
  ended up within a unit of their neighbours' line (AC 24, including the tip
  of a needle thinner than the grid) only while all input vertices of the run
  stay within a unit of the new line (AC 24a, 24b). After the fix the same
  circles deviate by under one grid unit from their input (0.0007 mm at
  100,000 vertices, which collapse to 1,892 nodes). A round's bound is one
  unit; the repair union can create a few vertices that further rounds (at
  most four) remove, each within the same bound of the round's input, so the
  worst case is four units (0.004 mm) in theory, one in practice. Slivers go
  inside the loop because dropping one can turn a touching vertex of its
  neighbour into a plain collinear one.
- **Exclusion** folds the engine's XOR pairwise.
- **Operand order.** Intersection and Exclusion fold pairwise, so the result
  can differ with the operand order by rounding within the grid tolerance
  (areas agree to the bound of criterion 14, nodes may differ). Union is
  exactly order-independent in the property tests; Difference is ordered by
  definition.
- **Criterion 14:** the PO reworded it to
  `1e-6 * (|A| + |B|) + 0.001 mm * (perimeter A + perimeter B)`, because each
  crossing is rounded to the grid and a pure 1e-6 cannot hold at the size of a
  laser bed. The kernel tests assert that bound on shapes up to 60 mm and up
  to 6 m, for union, intersection, difference and exclusion.
- **Performance** (release, `boolean_performance.rs`): 2 x 1,000 curved nodes
  take 5 to 7 ms, 2 x 10,000 take 49 to 63 ms per operation (budgets: 100 ms,
  2 s). The tests assert them in release builds only; the CI job
  `boolean-budgets` runs them there.

### 2026-10-09: what PR 2 settled (implementer)

- **The encoding is as decided above**, in a new `subpath_codec.rs`; `path_codec.rs` only calls it.
  `extra_subpaths` is created with `insert_container` like `anchors` (a boolean result is written once
  by one peer, so the mergeable form of the style stops is not needed). `format_version` is 8.
- **`replace_with_path`** lives in `replace.rs`, takes the label as a `&str`, returns the new
  `NodeId`, and refuses with `NoSuchObject`, `BaseNotAnOperand`, `NoOutlines` (none, or an outline
  without an anchor) or `AnchorIds` (an id twice). It takes the base's whole style by `read_style` /
  `write_path_style`, so a rectangle base works as well as a path base.
- **Measured write cost** (release, this machine): 20,000 anchors in four outlines are written in
  199 ms, read back in 23 ms, saved in 203 ms (1.6 MB) and reopened in 55 ms; replacing 1,000
  objects by a result of 4,000 anchors takes 37 ms. That meets criteria 46 and 47 with room, so the
  packed-coordinate encoding stays rejected.
- **`contains_point_in_outlines`** replaces `contains_point` (one function, any number of outlines; the
  0007 tests call it with one).
- **Compound paths leave the Node tool at one place**, `Session::paths()`; `check_join`,
  `check_split` and the commands refuse independently. A double-click on one in the Select tool is
  the new `SelectDoubleClickOutcome::CompoundPath` (no handoff, nothing changes); the session maps it
  to "no edit hint" until PR 3 shows the sentence of criterion 38.
- **Stroke of a compound path** is one lyon path per outline after dashing, concatenated and
  tessellated as one layer, so a translucent stroke has no dark spot where two outlines meet.
- **Four older tests pinned the literal version 7** (the `0015` and `0007` slices' "no bump"
  checks); they now name the build's current version or the constant of this story.

### 2026-10-09: what PR 3 settled (implementer)

- **The command is `Session::apply_boolean(op)`**, only with the Select tool active and no drag in
  flight (`Ignored` otherwise). It plans in `curvyo-ui-core::boolean::plan_boolean` (operands in
  stacking order, base = lowest, or the top one for Reverse difference, outlines from snapshots),
  calls the geometry kernel, and writes with `Document::replace_with_path` in one commit labelled
  `boolean_<op>`. The result is the only selected object. Refusals carry codes and counts only
  (`NeedsTwo`, `OpenPaths`, `NoArea`, `OutOfRange`, `Empty`); the sentences are in
  `frontend/src/lib/booleanText.ts`.
- **Criterion 1 over the design system's "any active tool".** The spec enables the buttons only
  with the Select tool; the design-system row says "any active tool". The spec wins (it is the
  accepted text and the object selection is not drawn in the other tools). The tooltip adds "Use the
  Select tool." in that case, as criterion 1 says. The design-system row should be corrected.
  The Pen note "Finish the path first." therefore never shows: with the Pen active the buttons are
  dimmed for the tool reason.
- **No "working..." notice.** The kernel runs on the UI thread (no off-thread decision exists), so
  there is nothing to show after 150 ms that could be painted. The busy state (criterion 47a) is
  set, two frames are painted, then the call runs: wait cursor on `<html>`, `aria-busy` on the
  toolbar, and a capture-phase listener that swallows presses and keys until the call returns.
- **Refusal outline** is a draw-list layer built from the objects the session remembers as
  offenders; it is valid only while the Select tool is active and the selection is the one the
  refusal was made for, so a selection or tool change removes it without a call from the host. The
  host also calls `clear_boolean_refusal` when the notice ends. Not stored, not selectable.
- **Out of range** (coordinates the 0.001 mm grid cannot hold) has no text in the specification.
  Written: "<Operation> cannot handle objects this large or this far from the page. Nothing was
  changed." The customer or UX reviewer may reword it.
- **Compound path texts.** "Compound path" is the Rust subject line (`style_scope::kind_name`).
  `NodeToolbarState.compound_only` and the double-click code `"compound_path"` come from Rust; the
  sentence of criterion 38 is the one constant `COMPOUND_NODES_TEXT`. The Markers block rule of the
  design system needs no change: there is no Markers UI yet.
- **Frontend shape.** `BooleanCommands.tsx` (section, tooltips, notice) and `BooleanGlyphs.tsx`
  are hosted by `ToolRail`; `useBooleanCommands` holds availability, busy and the notice. The
  `Tool` enum does not grow. `useEditorSession` gained only `applyBoolean`, the `compoundOnly`
  field and the compound flag of the edit hint.

### 2026-10-09: where the code lives

- **`curvyo-geometry-core`:** `boolean` (operations, normalization, fold),
  `boolean_grid` (grid conversion, overlay calls, canonical form),
  `boolean_cleanup` (near-collinear removal, repair, slivers) and `flatten`;
  each under 500 lines. `contains_point` takes several outlines (PR 2).
- **`curvyo-document-core`:** the encoding above and one command,
  `Document::replace_with_path(operands: &[NodeId], base: NodeId,
  outlines: &[(Vec<NewAnchor>, bool)], label)`, in a new module (`objects.rs`
  is at 753 lines). It resolves every id first, then creates the result
  node directly after `base` (`mov_after`, as `duplicate_objects` does),
  writes `base`'s style with `write_path_style` (not a key-by-key copy, as
  Split decided), rotation 0 (AC 27), deletes the operands, and commits once
  (AC 19, 21, 22, 28). It does not compute geometry; `document-core` does not
  depend on the kernel.
- **Commit label:** `boolean_union`, `boolean_difference`,
  `boolean_intersection`, `boolean_exclusion`, `boolean_reverse_difference`,
  following the existing snake-case labels (`convert_to_paths`,
  `duplicate_objects`). The undo slice maps labels to display text. **For the
  PO:** AC 28 should say "a label that names the operation" instead of quoting
  "Union".
- **`curvyo-ui-core`:** a new `boolean` module: operands in z-order from the
  selection, base operand, outlines from each `ObjectSnapshot` (paths as
  stored, primitives via `outline_of_rotated`, through one shared helper that
  `hit_test_object` also uses), the kernel call at a named constant
  `BOOLEAN_TOLERANCE` = 0.01 mm, ids from `AnchorIdMinter`, the refusal types
  with the counts AC 15 to 17 need, the selection afterwards (AC 23), the
  `BooleanOp` enum and `boolean_availability` (next section). Split into
  `boolean` and `boolean_availability` if it passes 500 lines. The Select bar
  does not change (`select_bar.rs`, `SelectBarState` untouched). Nothing goes
  into `select_tool.rs` (2,502 lines) or `node_tool.rs` (1,809).
- **`curvyo-editor-wasm`:** a `session/boolean.rs` glue module (`Session` is
  already past the size limit, technical debt entry) and a `wasm_boolean.rs`
  binding.
- **No new crate.**

### 2026-10-09: boolean commands in the tool rail (customer decision 3)

**Commands are not tools.** The Rust `Tool` enum
(`curvyo-editor-wasm/src/session/mod.rs`) and the TS `Tool` type
(`frontend/src/hooks/useEditorSession.ts`) do **not** grow. A command button
never changes the active tool, is not `aria-pressed`, and never goes through
`onSelect(tool)` or `Session::set_tool`. Rejected: a `Tool::Boolean` mode with
its own bar. It is the Select bar again under another name, a mode the user
has to leave again, and it would claim a tool letter in the key table.

**Where the enabled state is computed.** One pure function in
`curvyo-ui-core::boolean`, never in the frontend and never in editor-wasm
(ADR 0001 §1 and §3):

```text
boolean_availability(objects: &[ObjectSnapshot], selection: &ObjectSelection)
  -> BooleanAvailability
BooleanAvailability = NeedsTwo                          greyed out
                    | OpenPaths { open: usize, of: usize }  enabled, refusal on activation (AC 15)
                    | Ready
```

- One state for all five buttons: their preconditions are identical.
- Counts only selected ids the document still holds (as `ids_of_kind`).
- AC 16 (operand without area) and AC 17 (empty result) are not part of the
  state: they need the kernel, so they are refused on activation.
- `plan_boolean` checks the same precondition function first, so a button
  state and the command cannot disagree.
- Whether `OpenPaths` looks different from `NeedsTwo`, and whether a greyed
  button is native `disabled` or `aria-disabled`, is the ux-engineer's call.
  The state keeps the two apart so the frontend never has to decide.
- The frontend reads it in `syncFromSession`, next to `selection_count()`,
  as a read-and-free plain object (the `NodeToolbarState` pattern), and only
  renders it.

**Which selection counts, per active tool.** `Session` passes its object
selection only while the Select tool is active, otherwise an empty one: the
rule `selected_for_keys` (`session/keys.rs`) already applies to the key
table. One private `Session` helper serves both.

| Active tool | Object selection today | Buttons |
|---|---|---|
| Select | drawn | `NeedsTwo`, `OpenPaths` or `Ready` |
| Node, Pen | kept by `set_tool`, not drawn; Node shows its node selection | `NeedsTwo` |
| Rectangle, Ellipse, Polygon/star | cleared by `set_tool` | `NeedsTwo` |

After a shape is drawn, the tool hands over to Select with that one shape
selected, so the buttons stay greyed until a second object is added.
Rejected: enabling the buttons in the Node tool on the kept object
selection. The command deletes objects and there is no undo yet; it must act
only on a selection the user can see, and in the Node tool the visible
selection is nodes, which can span other paths.

**Dispatch.** Button → `apply_boolean(op: &str) -> String` in
`wasm_boolean.rs` (an unknown `op` is a `JsValue` error, as
`convert_selected(kind)`) → `Session::apply_boolean(BooleanOp)` in
`session/boolean.rs` → `curvyo_ui_core::plan_boolean(objects, selection, op,
minter) -> Result<BooleanPlan, BooleanRefusal>` →
`Document::replace_with_path` → selection = the result (AC 23).

- `BooleanOp` lives in `ui-core` with five variants, the user's operations.
  It maps to the kernel's four operations plus operand order (Reverse
  difference = `Difference` with the top-most operand first) and to the
  commit label.
- `Session::apply_boolean` returns `"ignored"` unless the Select tool is
  active and no drag is in flight. It then flushes the Select bar and style
  previews and cancels an open entry (as `convert_selected_to_paths` does),
  plans, and writes once. Outcomes are `"applied"`, `"ignored"` or a refusal
  kind; refusal counts come from the read-and-free state above. The notice
  text lives in the frontend, keyed by refusal kind (as `KEY_HINT_TEXT`).
- The tool stays Select after the command.

**The rail.** `ToolRail.tsx` keeps its `ToolButton`s unchanged and renders,
under a divider, a command group from a new component (e.g.
`frontend/src/components/BooleanCommands.tsx`): plain buttons of the same
40 px size, props `availability`, `onCommand(op)`, `onReturnFocus`. New
tools are appended to the tool group above the divider, so the command group
moves down with them. Tab order, grouping and roving focus are the
ux-engineer's.

**Focus and key gating.** Nothing in the key gate changes.

- The rail is outside the canvas container that owns `onKeyDown`
  (`Canvas.tsx`), and `isFormControl` matches `button`. So while a rail
  button has focus, no canvas key fires and Space or Enter activates the
  button natively.
- After a mouse click, focus returns to the canvas (`onReturnFocus` when
  `event.detail > 0`, as `ToolButton` does), so Delete and the tool letters
  keep working. Keyboard activation keeps focus on the button.
- During a canvas drag the rail cannot be reached (pointer captured, focus
  in the canvas); the in-flight check in `apply_boolean` is a guard only.
- **No keyboard shortcuts:** `decide` in `session/keys.rs` gets no boolean
  action (AC 3).

**Height.** Six tools, a divider and five commands come to about 495 px
(11 × 40 px buttons, 4 px gaps, 4 px padding). With rulers (0015) the
viewport at an 800 × 600 window is about 546 px high, so the rail fits with
about 40 px to spare at its 12 px inset. No overflow rule now (no seventh
tool exists); flagged to the ux-engineer to measure.

## PR split and order

Updated 2026-10-09 for the customer decisions.

1. **Kernel.** `geometry-core` only, plus the workspace dependency. AC 7, 8,
   10 to 14, 17, 24 to 26, 39 to 45 at kernel level, fixtures in
   `tests/fixtures/`. Four kernel operations cover all five commands.
   Unchanged; in progress (`story/boolean-operations`).
2. **Compound path, format and document command.** Encoding, read model,
   the audit table, format bump (`main` + 1 at merge, 8 if nothing else
   bumps first), the `document-core` command `replace_with_path`, fixtures.
   AC 19 to 22, 27, 28, 30 to 38 and the write-cost measurement. No UI.
3. **Command and rail UI.** `ui-core`: `BooleanOp` (five), `plan_boolean`,
   refusals, `boolean_availability`, selection after; `editor-wasm`:
   `session/boolean.rs`, `wasm_boolean.rs`; frontend: the rail command group.
   AC 1 to 6, 9, 15 to 18, 23, 29, 46, 47 as the PO rewrites them for the
   rail. No Select bar change.
4. **Preview** (P1 to P3), only if question 6 is A. Kernel output drawn with
   the existing `--preview-new` live-preview path; no document write.

**Parallel plan with `document-size-and-rulers` (re-confirmed 2026-10-09).**
Rulers takes no `format_version`, so the version number no longer orders
the two features. `CLAUDE.md` §4 forbids parallel work on shared crates:

| Step | Booleans | Rulers | Shared crates |
|---|---|---|---|
| now | PR 1 (`geometry-core`) | PR 1 (`document-core`, `ui-core`) | none: in parallel |
| next | PR 2 | waits | `document-core` (`objects.rs` move helper), `ui-core` (`object_bounds.rs`), `render-core`, `editor-wasm` |
| then | waits | PR 2 (rulers, pasteboard, viewport) | `ui-core`, `render-core`, `editor-wasm`, `frontend` |
| then | PR 3 | waits | `ui-core`, `editor-wasm`, `frontend` (`App.tsx`) |
| last | | PR 3 (panel; after 0017 if 0017 goes first) | |

Booleans PR 2 starts only after rulers PR 1 merges, because both edit the
move helper and `object_bounds`. It goes before rulers PR 2 because it
carries the write-cost measurement that could still reopen the encoding.
Booleans PR 3 goes after rulers PR 2, because PR 2 moves the rail into the
ruler viewport (`App.tsx`) and the rail height above is measured there.
Swapping booleans PR 2 and rulers PR 2 costs nothing but that risk. Whichever
merges second adds the compound-path case to the rulers resize and fit tests
(rulers AC 17, 23).

## Flagged to the lead

1. ~~Customer, question 1~~: answered 2026-10-09, option A, format change
   approved.
2. ~~**PO:**~~ done 2026-10-09 (AC 1, 1a, 2, 3, 15, 22a, 23, 28, 39, 47a reworded): reword AC 39 (grid wording above) and AC 28 (label). Rewrite AC 1
   and 2 for the rail: always shown; greyed with fewer than two selected
   objects or with a tool other than Select active; the open-path state of
   AC 2 stays. AC 3 stands.
3. ~~**ux-engineer:**~~ done 2026-10-09 (UX notes and design system updated): the rail command group (divider, tab order, greyed vs.
   dimmed look, tooltip text for "select two or more objects with the Select
   tool"), and the rail height at an 800 × 600 window with rulers (about
   495 px of 546 px). Remove the "Boolean group" row from the Select bar
   layout in `docs/design-system.md`.
4. **`stroke-markers`:** needs a rule for markers on compound paths.
5. **Risk:** AC 46/47 depend on Loro write cost; measured in PR 2 before the
   encoding merges.
