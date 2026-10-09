# ADRs for "Boolean operations"

This slice adds the first real kernel operation and the first document-model
change since `stroke-and-fill-styling`: a path object may hold more than one
outline. **No new crate, no new ADR, one new external dependency
(`i_overlay`; the 2026-10-04 spike chose `clipper2-rust`, replaced on
2026-10-09, see "what PR 1 settled"), one `format_version`
bump.** The compound-path decision below needs the customer's approval
(`CLAUDE.md` §3, document model) before PR 2 starts; PR 1 does not depend
on it.

## Depends on

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

## Feature-local decisions

### 2026-10-09: compound path = option A, one object with several outlines (`needs-customer`)

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
  drafts claim 8 and 9 provisionally, and `document-size-and-rulers` may take
  one at its own merge). Migration from older versions: none, the key is
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
  with the counts AC 15 to 17 need, the selection afterwards (AC 23). The
  select bar only gets an availability flag. Nothing goes into
  `select_tool.rs` (2,502 lines) or `node_tool.rs` (1,809).
- **`curvyo-editor-wasm`:** a `session/boolean.rs` glue module (`Session` is
  already past the size limit, technical debt entry).
- **No new crate.**

## PR split and order

1. **Kernel.** `geometry-core` only, plus the workspace dependency. AC 7, 8,
   10 to 14, 17, 24 to 26, 39 to 45 at kernel level, fixtures in
   `tests/fixtures/`. Independent of question 1.
2. **Compound path.** Encoding, read model, the audit table, format bump,
   `replace_with_path`, fixtures. AC 19 to 22, 27, 28, 30 to 38 and the
   write-cost measurement. Starts after the customer answers question 1 (or
   on the default A).
3. **Command and UI.** `ui-core` entry, refusals, select bar group, wasm,
   frontend. AC 1 to 6, 9, 15 to 18, 23, 29, 46, 47. Exclusion and Reverse
   difference go here if question 2 is A (no extra kernel work).
4. **Preview** (P1 to P3), only if question 6 is A. Kernel output drawn with
   the existing `--preview-new` live-preview path; no document write.

**Overlap with `document-size-and-rulers`.** That story touches
`document-core` (size command, format version, fixtures), `ui-core` (ruler,
fit-to-content over `object_bounds`), `render-core` (pasteboard),
`editor-wasm` and `frontend/`. Only **PR 1** shares no crate with it and can
run in parallel. PRs 2 to 4 share every crate except `geometry-core`, so per
`CLAUDE.md` §4 they start after rulers merges. Merge order matters in two
places: `CURRENT_FORMAT_VERSION` (whichever merges first takes `main` + 1,
the other rebases and renumbers its fixtures and notes), and `object_bounds`
(PR 2 makes it cover all outlines; fit-to-content then covers separate
pieces of a compound path with no rulers change).

## Flagged to the lead

1. **Customer, question 1:** A (one object, several outlines; recommended,
   default) or B (one object per outline). A changes the file format; it
   implements ADR 0002 §6 as accepted, so no new ADR.
2. **PO:** reword AC 39 (grid wording above) and AC 28 (label).
3. **`stroke-markers`:** needs a rule for markers on compound paths.
4. **Risk:** AC 46/47 depend on Loro write cost; measured in PR 2 before the
   encoding merges.
