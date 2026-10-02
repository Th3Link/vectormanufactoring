# ADR 0003: Geometry kernel, boolean operations, offsetting and V-carving

**Status:** Accepted (customer sign-off, 2026-10-02)

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
   the spike picks between two named candidates and does not reopen it, so its
   result is recorded as a dated feature-local decision in that story's
   `specs/<feature-slug>/adrs.md` rather than by editing this ADR. A result
   rejecting *both* candidates would need a new ADR superseding this one.
4. **Offsetting** is built on the same two pieces: expand each contour and
   union the pieces (`kurbo` stroke expansion for the geometry, the boolean
   crate for the union), with a dedicated module and golden-file tests.
   Offsetting is not a thin wrapper around booleans in practice — mitered,
   rounded and bevelled joins, self-intersection cleanup and inner-offset
   collapse each need explicit tests.
5. **V-carving uses an approximate medial axis derived from a constrained
   Delaunay triangulation** of the flattened boundary (`spade`, pure Rust,
   robust predicates), not an exact Voronoi diagram of curve segments. Carve
   depth at a point is the distance to the boundary divided by
   `tan(half_angle)` of the V-bit, clamped to the configured maximum depth.
   This is the pragmatic reading of the customer's "Voronoi-based V-carving":
   the medial axis is what the algorithm actually needs, and an exact
   curve-segment Voronoi (OpenVoronoi's reason for existing) buys accuracy we
   can also get by flattening more finely.
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
