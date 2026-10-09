# Architect review: PR #69 (boolean kernel, PR 1 of 4), head c1b3e97

Date: 2026-10-09. Static review plus PR CI (all 13 checks green on c1b3e97,
including core-wasm32, deny, and the three-OS host gate). No local build was run.

## Verdict: CHANGES REQUESTED (documents only; the code is approved as it stands)

The code is sound. Specifically:
- The dependency: `i_overlay =9.0.1` with `default-features = false`. Its
  `default` feature set is empty, and `allow_multithreading` (rayon) is off.
  `cargo tree --target wasm32-unknown-unknown` shows only `i_float`,
  `i_shape`, `i_key_sort`, `i_tree` and `libm`, so nothing from the
  ADR 0011 §6 ban list.
- Licenses: MIT OR Apache-2.0, plus MIT for the helper crates and `libm`.
- MSRV is 1.88, below the workspace's 1.90.
- `#![forbid(unsafe_code)]` stays in our crate.
- `thiserror` error type: every offending operand index is listed.
- `BooleanResult` is independent of the compound-path storage.
- Own flattening uses only `+ - * /` and `sqrt`. The step-count bound and the
  hull early-exit are correct.
- Integer grid, with i128 used for the shoelace and collinearity products.
- No float adapter. `HashMap` is not used.
- Canonical order and winding are implemented.
- Pairwise fold for Intersection and Exclusion; Reverse difference is
  Difference with the operands reordered by the caller.
- Every module is under 500 lines and every function under 60.

The blocking findings are about the decision record: two open PRs and the
ADR index disagree with each other and with the code.

## Blocking

1. **PR #70 names the wrong version and the wrong integer engine. It also
   conflicts with this PR.**
   - Files: `docs/adr/0003-...md` and `specs/0016-boolean-operations/adrs.md`
     on branch `chore/adr-0003-kernel-library`.
   - Rule: the ADR must record what ships (the file format and dependency
     record is hard to change).
   - #70 says `=9.0.0`, "the `i32` integer API", and "`OutOfRange` shrinks to
     fit `i32`". The code pins `=9.0.1` and calls `Overlay::<i64>`
     (`boolean_grid.rs:82`). The i_overlay docs (`core::integer`) give the
     i64 engine a range of ±2⁶² units.
   - The bound `MAX_COORDINATE_MM = 1e7` mm is 10¹⁰ units. That is about 4×10⁸
     times inside the i64 range. AC 42's 100,000 mm is 10⁸ units, so the
     bound does **not** shrink. `huge_coordinates` already tests the edge of
     the bound.
   - Fix: the ADR amendment and `adrs.md` name **`i_overlay =9.0.1`** (the
     pinned and tested version; the 9.0.0 figures stay as spike/harness
     evidence only). They say "the i64 integer engine, `OutOfRange` at 10⁷ mm
     unchanged" and "default features off (the default set is empty)".
   - #70 also changes the same lines of `adrs.md` that this PR changes.
     `git merge-tree` reports a content conflict in
     `specs/0016-boolean-operations/adrs.md`.
   - Fix: drop #70's `adrs.md` hunk. This PR's "what PR 1 settled" note is the
     feature record, and #70 keeps only the ADR 0003 amendment pointing to it.
     Merge #70 before or together with #69.
   - Reconcile the `unsafe` count: 49 in #70 and in ADR 0003's spike note
     (`i_overlay` 9.0.0 alone), about 130 in this PR (five crates). State one
     figure with its scope.
2. **`adrs.md` "the kernel" still describes the Clipper pipeline.**
   - File: `specs/0016-boolean-operations/adrs.md`, section "2026-10-09: the
     kernel".
   - Rule: `adrs.md` must say what the feature decided. "Read Clipper as the
     overlay engine" does not cover steps that no longer exist.
   - Stale text:
     - Step 3: "into `Clipper64`" and "Clipper's range".
     - Step 6: "Clipper's `SimplifyPaths`, then one union, then drop". The
       actual step is the own collinear removal, a repair union and the
       sliver drop, in up to 4 rounds.
     - Determinism: "PR 1 checks kurbo" (that check is done).
     - Robustness: "A Clipper error maps to a typed `BooleanError`".
       `i_overlay` returns no errors.
     - The "what PR 1 settled" note lists a `KernelFailed` variant that does
       not exist (`boolean.rs:70-89`).
   - Fix: rewrite steps 3 and 6 and the two paragraphs to match the code
     (`boolean.rs` module doc, steps 1 to 7, is accurate), and delete
     `KernelFailed`.
3. **ADR index is stale.**
   - File: `docs/adr/index.md`.
   - Rule: the architect keeps the index tables current.
   - The 0003 row says "Accepted" with no amendment. The open-issues bullets
     (lines 102-117) still say the spike result "is `clipper2-rust`". The §4
     offsetting bullet rests on Clipper2's offsetter.
   - Fix (in #70): add "§3 amended 2026-10-09 (`i_overlay`)" to the status
     column, strike or replace the two bullets with "§4 stands; whether
     `i_overlay` offsets is checked with the offsetting story".
4. **Criterion 14 must be reworded before merge.**
   - File: `specification.md`, AC 14.
   - Rule: Definition of Done (criteria covered by tests). The tests assert
     `1e-6·(|A|+|B|) + 0.001 mm·(perimeter A + perimeter B)` on shapes up to
     60 mm. The criterion as written cannot hold there, and the analysis in
     `adrs.md` is right.
   - Fix (PO): reword AC 14 to that bound. Then AC 14 is the spec's
     definition, not a known deviation, and the first new
     `technical-debt.md` entry reduces to a one-line known limitation, or is
     deleted, since there is nothing left to resolve.

## Non-blocking

5. **`boolean_grid.rs` module doc names four jobs joined by "and".**
   - Rule: §5, one responsibility per module. My own `adrs.md` layout grouped
     them, so this is not blocking.
   - Fix: move `remove_near_collinear`, `is_removable`, `simplify`, `cleanup`,
     `without_slivers`, `is_sliver` and their tests (about 150 lines) to
     `boolean_cleanup.rs`. `boolean_grid` keeps snap, overlay calls, area,
     canonical form and conversion back to mm.
6. **`area_identities_hold_within_one_millionth_on_large_shapes` is known to
   fail on 1 of 20,000 pairs.** It passes only because of the fixed seed. A
   pin update that moves one crossing can turn it red for no real defect.
   Fix: after the AC 14 rewording, assert the reworded bound there too.
7. **Property case count is hard-coded to 256.**
   - `boolean_properties.rs:35` overrides `PROPTEST_CASES`. Because of that,
     the 19,643-pair lattice run that justifies the library change cannot be
     repeated without editing code, even though `adrs.md` and the
     `Cargo.toml` comment say a pin update "reruns the property tests".
   - Fix: take `cases` from `PROPTEST_CASES` when set (default 256). Write the
     rerun command (for example `PROPTEST_CASES=20000 cargo test --release -p
     curvyo-geometry-core --test boolean_properties`) into the `adrs.md`
     library bullet.
   - CI cost is fine as is: 256 cases per property in debug, inside the host
     gate.
8. **The golden results are self-generated and not checked against AC 41.**
   - The `CURVYO_UPDATE_GOLDEN` regeneration mode is fine (no ignored
     generator needed; diffs are reviewed).
   - Fix: move `check_invariants` from `boolean_properties.rs` to
     `tests/common` and apply it to every golden `Ok` result in
     `every_fixture_matches_its_golden_result`. The bow-tie, sliver and
     duplicated-node fixtures are where AC 41 matters most.
9. **AC 43 in the browser is argued but not tested.** CI runs the goldens on
   Linux, macOS (aarch64) and Windows. The wasm32 job only builds and lints.
   The argument (IEEE basic operations, `sqrt` and `round` only, no FMA
   contraction in Rust) is sound. Fix: either cover it in PR 3 (run the golden
   inputs through the wasm build) or record it as a debt entry.
10. **The cleanup's deviation is not bounded when vertices are removed in a
    chain.**
    - The stack pass in `remove_near_collinear` checks each removed vertex
      only against its neighbours at the time of removal. A chain of
      removals can drift more than one unit from the final chord.
    - The budget for this is 0.002 mm minus 0.0007 mm of snapping.
    - This is unlikely on flattened curves (their vertices are far from
      collinear), and AC 25 is tested only on two discs.
    - Fix: either state the bound in a doc comment with its argument, or add
      a property test of AC 25 on random curved operands.
11. **The `boolean-budgets` CI job is fine as it is.**
    - Correctly scoped: one crate, one test target, release, ubuntu only.
    - Already uses `Swatinem/rust-cache@v2`. Most of the 1m48s is building
      `loro` in release.
    - Margins are 15× (1,000 nodes) and 28× (10,000 nodes), so it will not
      flake.
    - It belongs in `ci.yml` as its own job (a release target cannot share
      the host gate's debug cache).
    - The lead should add it to the required checks of branch protection,
      otherwise a budget regression does not block a merge.
12. **`technical-debt.md`, `unsafe` entry:** align the site count with
    finding 1. The entry is otherwise good.

## Version answer for the lead

ADR 0003's amendment and `adrs.md` must name **`i_overlay =9.0.1`**. That is
what `Cargo.toml`/`Cargo.lock` pin, and what the 19,643 + 19,999 kernel pairs,
the goldens and the budgets ran on. 9.0.0 appears only as the spike and harness
version in the evidence.
