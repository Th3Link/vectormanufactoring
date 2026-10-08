# ADRs for "Boolean operations"

This slice adds the first real kernel operation and the first document-model
change since `stroke-and-fill-styling`: a path object may hold more than one
outline. **No new crate, no new ADR, one new external dependency
(`clipper2-rust`, chosen by the 2026-10-04 spike), one `format_version`
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
  `clipper2-rust` (spike note under §3, 2026-10-04). No backend trait.
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

- **Library:** `clipper2-rust`, as the spike decided (ADR 0003 §3 note).
  Pure Rust, `#![forbid(unsafe_code)]`, BSL-1.0 (already allowed in
  `deny.toml`, added for `loro`'s `xxhash-rust`, so no `deny.toml` change), one dependency (`num-traits`, MIT/Apache). The
  spike built it for `wasm32-unknown-unknown`. Added as a workspace
  dependency, `default-features = false`, used only by
  `curvyo-geometry-core`. Take the newest 1.x at PR time and pin it exactly
  (`=1.x.y`): its provenance risk (AI-assisted port) means an update is a
  reviewed PR that reruns the fixtures, not a Renovate auto-bump.
- **Input and output.** The kernel takes operands as lists of outlines
  (`&[OutlineTriple]` plus `closed`, the triple `curvyo-ui-core` already
  builds for hit-testing) and returns `Vec<Vec<Point>>`. No `kurbo` or
  Clipper type crosses the crate API (as today). It does not take
  `PathSnapshot`, so PR 1 does not wait for PR 2.
- **Operations:** `Union`, `Difference` (first operand minus the rest),
  `Intersection`, `Exclusion`. Reverse difference is `Difference` with the
  caller putting the top-most operand first; no fifth kernel operation.
- **Pipeline**, in this order:
  1. Refuse an open outline: `BooleanError::OpenOperand { index }` (AC 15;
     the kernel enforces it, not only the UI).
  2. Flatten every segment with `kurbo` to `tolerance − 2·GRID` (0.008 mm
     for the fixed 0.01 mm), leaving room for grid rounding and step 6.
     Disc of radius 10 mm: about 79 nodes, inside AC 26's 71 to 142.
  3. Snap to an integer grid, `GRID` = 0.001 mm (a named constant inside the
     kernel, not a setting), into `Clipper64`. Non-finite coordinates or
     |coordinate| > 10⁷ mm: `BooleanError::OutOfRange`. 100 000 mm is 10⁸
     grid units, far inside Clipper's range (AC 42).
  4. **Normalize each operand on its own:** a nonzero self-union. This
     gives every operand positive orientation and its painted region
     (self-intersections, AC 7; holes of a compound operand, AC 8). Without
     it, two operands wound in opposite directions would cancel in their
     overlap under nonzero. An operand that comes out empty:
     `BooleanError::EmptyOperand { index }` (AC 16).
  5. The operation: Union = one nonzero union of all; Difference = first
     operand as subject, the rest as clip; Intersection and Exclusion are
     **folded pairwise** (Clipper's subject/clip form computes
     `A ∩ (B ∪ C)`, not `A ∩ B ∩ C`, and its XOR is not odd-parity for
     three or more operands).
  6. Clean up: Clipper's `SimplifyPaths` with ε = 1 grid unit (AC 24), then
     one more nonzero self-union to repair any crossing the simplification
     made (AC 41), then drop outlines with fewer than 3 points or zero area.
  7. Canonical form (AC 41, 43): outer outlines have positive shoelace area
     in document coordinates (clockwise on screen, Y down), holes negative;
     each outline starts at its smallest grid point (x, then y); outlines are
     sorted by that start point, ties by signed area, larger first. Then back
     to mm (integer × 0.001).
  8. Empty result: `BooleanError::EmptyResult` (AC 17).
- **Determinism (AC 43):** Clipper64 is integer arithmetic; flattening and
  rounding use IEEE basic operations and `sqrt`, which agree on x86-64,
  aarch64 and wasm32. PR 1 checks that `kurbo`'s flattening path calls no
  `powf`/trigonometric function; if it does, the kernel flattens with its own
  subdivision. No `HashMap` iteration order anywhere in the kernel. Output
  lies on the grid, so the golden files agree exactly, not just to 1e-6 mm.
- **Robustness:** `catch_unwind` and timeouts are unavailable in a core
  crate (no threads, wasm aborts on panic), so the guard is the test suite:
  the 9 fixtures of AC 40, the 200-pair seeded property test of AC 14
  (`proptest` is already a dev-dependency), and the spike's four degenerate
  fixtures. A Clipper error maps to a typed `BooleanError`, never a panic.
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
  about 200,000 (well under 2 s, four Clipper passes included). **The risk is
  the document write, not the kernel:** every result node is a Loro map with
  five registers and a 32-character id, about 100 bytes in the operation log.
  AC 46 (1,000 rectangles) and AC 47 (150 ms to repaint) are dominated by that
  write and by reading the snapshot back. PR 2 measures writing and reading a
  20,000-anchor compound path before it merges, because the encoding is the
  hard-to-change part. If it misses, the fallback is the packed encoding
  rejected above, as a new decision here and not a silent change.

### 2026-10-09: where the code lives

- **`curvyo-geometry-core`:** `boolean` (operations, normalization, fold),
  `boolean_grid` (grid conversion, cleanup, canonical form) and `flatten`;
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
