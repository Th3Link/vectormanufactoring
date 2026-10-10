# ADR 0003: Geometry kernel, boolean operations, offsetting and V-carving

**Status:** Accepted (customer sign-off, 2026-10-02); §3 kernel library amendment of 2026-10-09 in effect since PR #69 merged on 2026-10-09 (`i_overlay` `=9.0.1` replaces `clipper2-rust`); §4 offsetting amendment of 2026-10-10 (customer decision: `i_overlay`'s integer offsetter, no new dependency)

## Context

The product needs, in roughly this order: boolean operations (union,
difference, intersection) on paths; stroke-to-outline conversion; path
simplification and smoothing better than Inkscape's; offsetting for laser
kerf compensation, CNC tool radius and embroidery borders; and V-carving of
text, which the customer names explicitly and associates with a Voronoi /
medial-axis approach (OpenVoronoi).

Hard constraint: this all lives in `*-core` crates, which must build for
`wasm32-unknown-unknown` and use `#![forbid(unsafe_code)]` (`CLAUDE.md`
§5, §6).

### Options considered

**A. C++ libraries via FFI — Clipper2 for booleans/offsetting, OpenVoronoi for
V-carving.** These are the best-tested implementations in the industry and
Clipper2 in particular is what most CAM software relies on. Rejected: FFI
requires `unsafe` in a core crate, building C++ (OpenVoronoi additionally
needs Boost) for `wasm32-unknown-unknown` is not a path we want to own, and
OpenVoronoi's LGPL licensing would constrain ADR 0006. The quality argument is
real and is the strongest case against this ADR; it is recorded as such.

**B. Pure-Rust kernel assembled from maintained crates.** Keeps the wasm and
`forbid(unsafe_code)` constraints intact, one toolchain, one build. Cost: less
battle-hardened than Clipper2, and we own robustness bugs on degenerate input.

**C. Write the kernel ourselves, curve-exact.** Curve-exact booleans preserve
Béziers through union/difference, which is what Inkscape does and what keeps
node counts low. Rejected: months of work with high robustness risk, and it is
not needed to cut material. Revisit as a refitting improvement, not a
foundation.

## Decision

1. **A pure-Rust geometry kernel**, `vecmanf-geometry-core`, is the only place
   geometric algorithms live. It takes and returns `vecmanf-document-core`
   path types, takes an explicit `Tolerance` on every operation, and has no
   global state.
2. **Curve math** (evaluation, derivatives, flattening to a tolerance, arc
   length, curve fitting, stroke expansion, single-curve offsetting) uses
   `kurbo` — pure Rust, MIT/Apache, wasm-clean, and the most mature Bézier
   library in the ecosystem.
3. **Boolean operations run on flattened polygons**, not on curves: flatten to
   the caller's `Tolerance`, run the boolean, return polylines. The polygon
   boolean implementation is a pure-Rust crate (`i_overlay` is the leading
   candidate; `geo`'s boolean ops are the fallback). **Which one is settled by
   a throwaway spike on `spike/booleans` before the first geometry story**,
   judged on degenerate input (self-intersections, coincident edges, zero-area
   slivers, thousands of subpaths) and on wasm build cleanliness. The decision
   accepted here is "flattened polygons, a pure-Rust crate, chosen by spike";
   the spike picks between the named candidates and does not reopen it, so its
   result is recorded as a dated feature-local decision in that story's
   `specs/<NNNN-feature-slug>/adrs.md` rather than by editing this ADR. A result
   rejecting *all* candidates would need a new ADR superseding this one.

   > **Note 2026-10-02 (candidate list only, decision unchanged):** the
   > customer pointed out `clipper2-rust`, which is a genuine pure-Rust port of
   > Clipper2 — not an FFI binding — by MatterHackers: `#![forbid(unsafe_code)]`,
   > one dependency (`num-traits`), BSL-1.0 like upstream, 1.2.0 of 2026-09-18
   > after six releases since 2026-02, and a wasm demo that establishes the
   > `wasm32-unknown-unknown` build. It is therefore a **third candidate for the
   > `spike/booleans` comparison** alongside `i_overlay` and `geo`, and it
   > carries the quality argument that option A below was rejected despite: the
   > algorithms are Clipper2's, without the `unsafe`, the C++ toolchain or the
   > license problem. Its open question is provenance, not safety — the port was
   > produced with AI assistance and is "maintained in spare time", so the
   > spike's degenerate-input fixtures are what decides it, not the test count
   > in its README. Note that it also ships Clipper2's own **offsetter**, which
   > §4 below builds differently; see the note on §4.
   >
   > This changes no accepted text. Option A stays rejected as written — it was
   > rejected for C++-via-FFI, and a pure-Rust port is option B, not option A.
   >
   > **Spike result, 2026-10-04 (`spike/booleans`, never merged — code
   > discarded, result recorded here):** `i_overlay` 9.0.0, `geo` 0.33.1
   > (`default-features = false`, `features = ["earcut"]`) and `clipper2-rust`
   > 1.2.0 were run through union/difference/intersection on four degenerate
   > fixtures — a self-intersecting bowtie, a contour with a duplicated point
   > (zero-length/coincident edge), a near-degenerate sliver (10×1e-9), and
   > 5,000 disjoint tiny-square subpaths — each under a 10s timeout with
   > `panic::catch_unwind` on a worker thread. **All three candidates handled
   > every case: no panic, no hang, no garbage output**, and their results
   > agreed with each other on every case (the sliver collapsing to empty on
   > difference/intersection in all three is consistent across a precision
   > sweep of 2/6/10/15 decimal places for `clipper2-rust`, so it is the
   > sliver's area being genuinely below double precision at that scale, not
   > a candidate-specific rounding bug). On 5,000 subpaths all three completed
   > intersection in 9–14ms with no distinguishing robustness gap. Each
   > candidate, isolated in its own crate with `default-features = false`,
   > also built cleanly for `wasm32-unknown-unknown` with no code changes
   > needed; `geo` pulls `rand`/`rand_pcg` transitively through its bundled
   > `i_overlay` 4.5.2 even with defaults off, but not `getrandom`, so nothing
   > on ADR 0011 §6's wasm32 ban list appears for any of the three.
   >
   > Degenerate-input handling and wasm cleanliness being a three-way tie, the
   > decision is **`clipper2-rust`**, on grounds the ADR's two stated criteria
   > don't capture but that are reasonable tie-breakers: it is
   > `#![forbid(unsafe_code)]` with zero `unsafe` in its own source (matching
   > `CLAUDE.md` §5 most directly), whereas `i_overlay` 9.0.0 uses `unsafe`
   > internally 49 times for performance (not FFI, but still unsafe Rust this
   > project would otherwise avoid) and `geo` 2 times; and its algorithms are
   > Clipper2's, the implementation this industry (including CAM/laser-cutting
   > tooling adjacent to this product) has relied on for over a decade, so the
   > port's job is transcription fidelity rather than novel robustness, which
   > this spike's agreement across all four fixtures corroborates. A note on
   > `geo`: it is not an independent third implementation here — `geo`'s own
   > `BooleanOps` is a wrapper over a pinned, older `i_overlay` (4.5.2 vs. our
   > direct 9.0.0), so picking it would mean inheriting `i_overlay`'s
   > unsafe-internals profile a version behind plus `geo-types`' ecosystem
   > weight and the transitive `rand` dependency, for no benefit this project
   > uses (manufacturing paths stay in `vecmanf-document-core` types, not
   > `geo-types`). `clipper2-rust`'s provenance question (AI-assisted port,
   > "maintained in spare time") is the one open risk this spike does not
   > fully retire — it is mitigated, not eliminated, by the cross-candidate
   > agreement above.
   >
   > This decision is `vecmanf-geometry-core`'s to apply once slice 6
   > (`boolean-operations`) exists as a story; this note is the dated
   > feature-local record the ADR's own text asks for, written here because
   > no `specs/<NNNN-feature-slug>/adrs.md` exists yet for that slice.
   >
   > **Amendment 2026-10-09 (kernel library: `i_overlay` replaces
   > `clipper2-rust`). Proposed by the lead as a default the customer may
   > veto; takes effect when PR #69 (`story/boolean-operations`) merges.**
   > The provenance risk above materialised. The kernel's property tests
   > (5,877 random pairs of 1 to 3 outlines on a 6 mm lattice, many shared and
   > collinear edges, area identities of AC 14) gave 12 to 16 violations with
   > `clipper2-rust` 1.2.0 and none with `i_overlay` 9.0.0; on unconstrained
   > random input (18,000 pairs) both had none. One reproducing case: a union
   > fills 0.4 to 0.5 mm² that no operand covers. Clipper's `SimplifyPaths`
   > also turned a valid outline into a self-crossing one. Evidence and
   > fixtures: PR #69, `docs/technical-debt.md` entry of that PR.
   >
   > - **Decision:** `i_overlay` **`=9.0.1`**, exact pin (9.0.0 is the
   >   spike and property-test evidence; 9.0.1 is what PR #69 pins and
   >   tests), workspace dependency, `default-features = false`
   >   (`allow_multithreading` pulls `rayon` and stays off), used only by
   >   `curvyo-geometry-core`. MIT OR Apache-2.0; its own dependencies
   >   `i_float`, `i_shape`, `i_tree` and `i_key_sort` are MIT. Rust 1.88
   >   minimum, below the workspace's 1.90. The spike built it for
   >   `wasm32-unknown-unknown`; nothing on ADR 0011 §6's ban list. An update
   >   is a reviewed PR that reruns the fixtures and property tests, not a
   >   Renovate auto-bump.
   > - **Determinism (AC 43):** the kernel snaps to its own fixed 0.001 mm
   >   grid and runs `i_overlay`'s `i64` engine (`Overlay::<i64>`, integer
   >   predicates, no platform math, range ±2⁶² units). It does **not** use
   >   the float adapter, which picks its scale from the input bounds and
   >   would make the grid depend on the input. The `OutOfRange` bound is
   >   unchanged: 10⁷ mm = 10¹⁰ grid units, far inside the range; AC 42's
   >   100,000 mm is 10⁸ units. The golden files and the cross-target check
   >   of AC 43 are the evidence.
   > - **Why `clipper2-rust` lost:** a wrong area is a wrong cut; no other
   >   criterion outweighs it. The spike's tie-breaker (`forbid(unsafe_code)`
   >   in the dependency) was only ever a tie-breaker. Unsafe count, scoped:
   >   **49 sites in `i_overlay` 9.0.0's own source** (the spike's count, the
   >   figure this ADR cites); about 130 across all five `i_*` crates PR #69
   >   pulls in. `CLAUDE.md` §5 forbids `unsafe` in our crates, and
   >   `curvyo-geometry-core` keeps `#![forbid(unsafe_code)]`. `geo` stays
   >   rejected (wrapper over an older `i_overlay`, see above).
   > - **Hard delete:** `clipper2-rust` leaves `Cargo.toml` and `Cargo.lock`
   >   in PR #69. No fallback path, no feature flag, no backend trait (§8). It
   >   can come back only by a new dated decision here that passes the same
   >   property tests. BSL-1.0 stays in `deny.toml` for `xxhash-rust`.
   > - **Consequences:** the library is local to `boolean_grid::run`; the
   >   kernel API, the pipeline of `specs/0016-boolean-operations/adrs.md`
   >   (grid range, normalize, fold pairwise, own cleanup, canonical form),
   >   the fixtures and the property tests are unchanged. The §4 open question about
   >   Clipper2's offsetter loses its premise: §4 stands as written. Whether
   >   `i_overlay` itself offers outline offsetting with the joins §4 lists is
   >   **to be checked** when the offsetting story is specified (crates.io
   >   metadata shows `i_shape` is a data-structure crate, not an offsetter).
   >   Offsetting and V-carving may still need other libraries; that is
   >   decided with those stories, not here.
   >
   > This stays inside the accepted decision ("flattened polygons, a pure-Rust
   > crate, chosen among the named candidates"), so it is an amendment note,
   > not a superseding ADR.
4. **Offsetting** is built on the same two pieces: expand each contour and
   union the pieces (`kurbo` stroke expansion for the geometry, the boolean
   crate for the union), with a dedicated module and golden-file tests.
   Offsetting is not a thin wrapper around booleans in practice — mitered,
   rounded and bevelled joins, self-intersection cleanup and inner-offset
   collapse each need explicit tests.

   > **Open question raised 2026-10-02, not settled here:** if the spike picks
   > `clipper2-rust`, its ported Clipper2 offsetter already implements exactly
   > the join and cleanup cases this item lists, and building them ourselves on
   > top of `kurbo` stroke expansion would be redundant work against a worse
   > implementation. Using it instead would be a change to this accepted item,
   > not a candidate-list update, so it needs a new ADR superseding this one
   > rather than a feature-local note — flagged to the lead, customer decision.
   > Until that happens this item stands as written.
   >
   > **Note 2026-10-10:** the spike's pick was replaced by `i_overlay` (§3
   > amendment of 2026-10-09), so this question no longer applies; this item
   > stands as written.
   >
   > **Amendment 2026-10-10 (offsetting uses `i_overlay`'s own offsetter;
   > customer decision of 2026-10-10 on `specs/0038-path-offset`).** The
   > check that the §3 amendment left open has been done. `i_overlay`
   > `=9.0.1` has integer outline offsetting (`IntOutlineOffset`: Bevel,
   > Miter and Round joins, holes, inner collapse handled by its own union),
   > integer stroke offsetting for open paths (`IntStrokeOffset`: Butt,
   > Square and Round caps) and a variable-width stroke with round joins
   > (`IntVariableStrokeOffset`).
   >
   > - **Decision:** offsetting runs on the flattened, snapped polygons of
   >   the boolean pipeline through those integer traits, with
   >   `MathMode::Integer`. That is deterministic for the same reason as §3.
   >   It replaces "`kurbo` stroke expansion plus union" in this item. It
   >   adds no dependency.
   > - **Not used:** the float adapters (`OutlineOffset`, `StrokeOffset`).
   >   They scale from the input bounds, which §3 forbids.
   > - **Why the written plan lost:** it builds the same joins and the same
   >   cleanup as our own code, on float output that would have to be
   >   snapped to the grid anyway.
   > - **Known difference:** past the miter limit, `i_overlay` clips the
   >   miter at the limit distance (SVG 2 `miter-clip`) and does not bevel
   >   it.
   >
   > What stays: polyline results, a dedicated module, golden and property
   > tests. Details: `specs/0038-path-offset/adrs.md`.
5. **V-carving uses an approximate medial axis derived from a constrained
   Delaunay triangulation** of the flattened boundary (`spade`, pure Rust,
   robust predicates), not an exact Voronoi diagram of curve segments. Carve
   depth at a point is the distance to the boundary divided by
   `tan(half_angle)` of the V-bit, clamped to the configured maximum depth.
   This is the pragmatic reading of the customer's "Voronoi-based V-carving":
   the medial axis is what the algorithm actually needs, and an exact
   curve-segment Voronoi (OpenVoronoi's reason for existing) buys accuracy we
   can also get by flattening more finely.

   > **Re-checked 2026-10-02 (confirms this item):** there is still no Rust port
   > of OpenVoronoi, so building V-carve depth ourselves stands. The closest
   > pure-Rust alternatives are `boostvoronoi` (Boost.Polygon's segment-site
   > Voronoi ported to Rust, BSL-1.0) and `centerline` on top of it, which does
   > medial-axis extraction. Neither replaces this item: `centerline` pulls
   > `rayon`, which does not build for `wasm32-unknown-unknown` without threads;
   > both are low-traffic; and `boostvoronoi`'s own README says bugs remain.
   > They are the escalation path if the CDT approximation proves insufficient
   > on real cuts — recorded in `docs/technical-debt.md`, not adopted here.
6. **Path simplification** is a first-class module, not a boolean by-product:
   curve refitting to a tolerance (`kurbo`), with the "combine several trace
   settings" requirement handled in `vecmanf-vectorize-core` and not here.
7. **Tessellation for display** is separate from the kernel and lives in
   `vecmanf-render-core` (`lyon`, ADR 0001). Display tessellation may be
   coarser than manufacturing geometry; they must not share a tolerance.
8. No "geometry backend" trait is introduced. There is one implementation
   (`CLAUDE.md` §5); if option A ever returns, it returns as a replacement
   recorded in a new ADR.

## Consequences

- The whole kernel builds for wasm and host with one toolchain and no
  `unsafe`; the browser target gets real geometry, not a stub.
- **Boolean results are polylines, not curves.** A union of two circles comes
  back as a dense polygon. For manufacturing this is harmless — everything is
  flattened before it reaches a machine anyway — but for further *editing* it
  is worse than Inkscape: node counts explode and the result is not cleanly
  re-editable. Mitigation is optional curve refitting after a boolean, which
  is a story, not part of this ADR. Recorded in `docs/technical-debt.md`.
- V-carve quality depends on flattening density, and the approximation will
  differ visibly from OpenVoronoi on sharp interior corners. Depth maps need
  golden tests plus a visual check against known-good reference cuts.
- We own robustness. Degenerate-input fixtures (from real customer files, not
  synthetic ones) are part of the kernel's test suite from the first story.
- One `Tolerance` type threaded everywhere is verbose. That is the point: a
  wrong tolerance is a ruined workpiece, so it stays visible at every call
  site.
