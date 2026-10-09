# Plan for Boolean operations

Branch `story/boolean-operations`, worktree `/home/marc/workbench/vecmanf-claude/boolean-kernel`, from
`main` at `2a83559`. This plan covers all four PRs of the architect's split (`adrs.md`, "PR split and
order"). Only PR 1 is written now; PRs 2 to 4 are listed so the kernel API fits what they need and
are marked **later**. PR 2 waits for the customer's answer to question 1 (default: option A) and,
per `CLAUDE.md` §4, for `0015-document-size-and-rulers` to merge, because it shares crates with it.
PR 3 and PR 4 start after PR 2. Each later PR gets its own branch and re-derives its task list from
this one when it starts.

## Affected crates/modules

| PR | Crate | New or changed |
|---|---|---|
| 1 | `curvyo-geometry-core` | New `flatten.rs` (cubic outline to polyline, deterministic), `boolean_grid.rs` (grid snap, cleanup, canonical form), `boolean.rs` (operations, normalization, pairwise fold, errors, result type). `lib.rs` re-exports. `tests/fixtures/` (golden files), `tests/boolean_*.rs`. |
| 1 | workspace and CI | `Cargo.toml` + `Cargo.lock`: `i_overlay` pinned exactly (it replaced `clipper2-rust` during PR 1, see `adrs.md`), `thiserror` for the kernel's error type (already a workspace dependency). `deny.toml` needs no change (`i_overlay` and its helper crates are MIT or MIT OR Apache-2.0). `.github/workflows/ci.yml` gets a `boolean-budgets` job that runs the performance tests in release. |
| 2 (later) | `curvyo-document-core` | `extra_subpaths` encoding, `PathSnapshot::subpaths()`, `subpath_codec`, the audit table of `adrs.md`, `Document::replace_with_path`, format bump, fixtures. |
| 2 (later) | `curvyo-geometry-core` | `contains_point` over several outlines. |
| 2 (later) | `curvyo-ui-core`, `curvyo-render-core` | Hit-testing, bounds, oriented box, fill, stroke and dash over all outlines; Node tool skips compound paths. |
| 3 (later) | `curvyo-ui-core`, `curvyo-editor-wasm`, `frontend/` | `boolean` module (operands in z-order, base operand, outlines per snapshot, `BOOLEAN_TOLERANCE`, refusals), `session/boolean.rs`, Boolean group in the Select bar, notices, red outline of offenders. |
| 4 (later, optional) | `curvyo-ui-core`, `curvyo-editor-wasm`, `frontend/` | Preview (P1 to P3) on the existing `--preview-new` path; no document write. |

## Tasks

### PR 1: kernel (`curvyo-geometry-core` only)

- [x] 1. Add `i_overlay` (exact pin, `default-features = false`) and `thiserror` to
  `curvyo-geometry-core`; verify `cargo deny check`, the wasm32 build and the banned-dependency
  tree check of `.github/workflows/ci.yml` (all AC; precondition).
- [x] 2. `flatten.rs`, tests first: a closed outline of cubic segments becomes a polyline whose
  chords stay within the given tolerance of the curve, using only `+ - * /` and `sqrt` so the
  result is bit-identical on every target (AC 25, 26, 43). Test: disc of radius 10 mm from four
  Béziers gives 71 to 142 nodes at 0.01 mm tolerance and every chord midpoint is within 0.01 mm of
  the circle.
- [x] 3. `boolean_grid.rs`, tests first: snap to the 0.001 mm integer grid (named constant),
  range check (non-finite or beyond 10⁷ mm refused), consecutive duplicate removal, canonical form
  (winding, start point, outline order) and conversion back to mm (AC 39, 41, 42, 43).
- [x] 4. `boolean.rs` types and refusals, tests first: `BooleanOp` (Union, Difference,
  Intersection, Exclusion), `Outline`, `BooleanResult`, `BooleanError` (no operands, tolerance too
  small, open operands, out of range, empty operands, empty result). Open paths and operands
  without area are refused with all offending operand indices, so the UI can give the counts and
  outline every offender (AC 15, 16, 17).
- [x] 5. `boolean.rs` pipeline, tests first: flatten, snap, normalize each operand by a nonzero
  self-union, run the operation (Union as one union; Difference as subject minus the union of the
  rest; Intersection and Exclusion folded pairwise), simplify with epsilon 1 grid unit, repair by
  a second nonzero union, drop degenerate outlines (AC 4, 7, 8, 10, 11, 12, 13, 24, 39, 41).
- [x] 6. Golden fixtures in `tests/fixtures/` for the nine shapes of AC 40, the ring plus island of
  AC 8, the two discs of AC 25, the AC 39 pairs, and the four spike fixtures of ADR 0003 (bow-tie,
  duplicated point, sliver, 5,000 tiny squares). Fixtures are plain text (`.fixture`): the operands as anchors in mm, then the expected result
  of each operation as grid integers, so they diff and agree exactly on every OS (AC 40, 43). The
  5,000-square case is generated in the test and stored as a summary with a hash.
- [x] 7. Property tests, `proptest` with a fixed seed: at least 200 random polygon pairs including
  holes and self-intersections; `|A ∪ B| + |A ∩ B| = |A| + |B|` and `|A − B| + |A ∩ B| = |A|`
  within 1e-6 · (|A| + |B|); idempotence of Union; commutativity of Union, Intersection and
  Exclusion; Exclusion area identity; determinism over repeated runs; output invariants of AC 41
  (at least 3 points, area > 0, no self crossing, finite, holes inside outers) (AC 14, 41, 43).
- [x] 8. Degenerate inputs: zero area, coincident edges, touching points, self-intersections,
  opposite winding operands, tiny and huge coordinates (up to 100 000 mm), NaN and infinity
  refused, the AC 39 grid pair (20.0000 / 20.0004 / 20.0020 mm), the AC 7 five-point star and the
  AC 8 ring (AC 7, 8, 16, 39, 40, 42).
- [x] 9. Performance tests: 2 × 1,000 curved nodes within 100 ms and 2 × 10,000 within 2 s for each
  of Union, Difference, Intersection (AC 44, 45). Asserted in release only (the CI job `boolean-budgets` runs them there); in debug the same cases
  run in about 0.5 s and print the numbers. Measured numbers go into the PR description.
- [x] 10. Docs: module docs (one responsibility each, files under 500 lines), the winding rule and
  the guarantees written down in `boolean.rs`; a dated note in `adrs.md` for what PR 1 settled
  (own flattening because `kurbo`'s uses `powf`; error lists instead of single indices; own
  cleanup; the library change and why). Run the
  full gate of `CLAUDE.md` §7 and every step of `.github/workflows/ci.yml` on the head sha.

### PR 2: compound path (later; needs customer answer to question 1 and `0015` merged)

- [ ] 11. `extra_subpaths` encoding, read model (`SubpathSnapshot`, `subpaths()`), open-file
  validation, `format_version` bump (`main` plus one at merge), golden files for a compound path
  and an unchanged older file (AC 30, 37, 37a).
- [ ] 12. The audit table, one test per line: `rotated`/`scaled`/`sheared`, `translate_objects`,
  `duplicate_objects`, resize, rotate and skew commits (AC 35, 35a, 36, 36a).
- [ ] 13. `contains_point` over several outlines; `ui-core` hit-test, bounds, oriented box
  (AC 33, 34).
- [ ] 14. `render-core`: fill, stroke, dash over all outlines (AC 31, 31a, 32).
- [ ] 15. Node tool skips compound paths, hint chip, Join and Split refuse, Markers block hidden
  (AC 38, 38a, 38b).
- [ ] 16. `Document::replace_with_path` (one commit, label, base operand's place and style,
  rotation 0) and the write-cost measurement of a 20,000-anchor compound path (AC 19 to 22, 27, 28,
  46, 47).

### PR 3: command and UI (later; after PR 2)

- [ ] 17. `ui-core` `boolean` module: operands in z-order, outlines from snapshots (primitives
  through `outline_of_rotated`), base operand, kernel call, refusals with counts, selection after
  (AC 4, 5, 6, 9, 15 to 17, 19, 23).
- [ ] 18. Session glue and Select bar Boolean group: one Tab stop, roving focus, tooltips, notices,
  busy state, red outline of offenders, focus to the canvas (AC 1, 2, 3, 18, 22a, 29, 47a).
- [ ] 19. Exclusion and Reverse difference buttons (question 2 default A) (AC 13).
- [ ] 20. Performance with the document write: 1,000 rectangles, button press to repaint
  (AC 46, 47).

### PR 4: preview (later, optional; question 6 default A)

- [ ] 21. Hover-intent and keyboard-focus preview, 2,000-node cap, "Would be empty" tooltip note
  (P1 to P3).

## Validation

- Golden files in `curvyo-geometry-core/tests/fixtures/`, committed, compared exactly (the kernel
  output lies on the 0.001 mm grid, so equality is exact, not within 1e-6).
- Property tests with a fixed seed (area identities of AC 14, idempotence, commutativity,
  determinism, output invariants of AC 41).
- Degenerate-input tests, including the four fixtures of the ADR 0003 spike. These and the
  property tests are the safeguard for the risk of a young polygon engine:
  an update of the pin is a reviewed PR that reruns them.
- Performance numbers measured in release and recorded in the PR; asserted only in release.
- Gate: `CLAUDE.md` §7 plus every step of `.github/workflows/ci.yml`, run locally on the exact head
  sha, then GitHub CI green on that sha.
