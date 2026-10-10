# ADR 0011: Workspace and crate layout

**Status:** Accepted (architect, 2026-10-02 — no customer sign-off required); crate and directory names amended by [ADR 0013](0013-rename-to-curvyo.md) (`vecmanf-*` is now `curvyo-*`); §7 CI note of 2026-10-10 (Windows and macOS host gate nightly only, customer decision 2026-10-09)

This ADR invents no architecture. ADRs 0001–0010 each authorized the crates
their own decision needed, in the ADR that needed them; this one pulls those
names into one workspace, fixes the dependency direction between them, and says
how the wasm32 requirement and the TypeScript frontend are enforced in CI. It is
the follow-up the ADR index named after 0001–0010 were accepted, and
`CLAUDE.md` §5 forbids creating a crate without an ADR, so no crate exists
before this one.

**Why this is not `needs-customer`.** `CLAUDE.md` §3's sign-off list is platform,
UI framework, document model, persistence/sync, plugin model, license and
accounts/payment. Every one of those is already settled in an accepted ADR with
customer sign-off. Crate layout is downstream assembly of those decisions and is
reversible by refactoring, so it is the architect's to accept — which is also why
it must not quietly change any of them, and the one place it comes close
(§3's plugin SDK dependency) is called out as such.

## Context

Eleven crate names and one frontend are committed to across the accepted ADRs.
Collected from their actual text rather than from the index's summary:

| Named in | Crate |
|---|---|
| 0001 §1, §3 | `vecmanf-ui-core` |
| 0001 §3, §4 | `vecmanf-render-core`, `vecmanf-editor-wasm` (authorized there) |
| 0001 §6 | `vecmanf-app` |
| 0002 §1 | `vecmanf-document-core` |
| 0003 §1 | `vecmanf-geometry-core` |
| 0003 §6 | `vecmanf-vectorize-core` |
| 0004 §5 | `vecmanf-sync-server` (authorized there) |
| 0004 §8, 0007 §5 | `vecmanf-library-core`, `vecmanf-storage-io` |
| 0005 §1 | `vecmanf-plugin` |
| 0008 §4 | `vecmanf-crypto-core` (authorized there) |
| 0001 §2 | the TypeScript/React/Tailwind frontend (not a crate) |

Three things the ADRs decided that constrain the shape, and that this ADR
therefore records rather than reopens:

- **ADR 0008 §5 is the single permitted exception to ADR 0004 §5's "the server
  depends on no `*-core` crate".** The server needs the cleartext routing-header
  parser, and sharing it beats a second implementation drifting. ADR 0010 §14
  widens the *contents* of that exception — keyring replay and fork-merge are
  pure, so client and server run the same code — without widening the exception
  itself. The reason behind 0004 §5 survives only if `vecmanf-crypto-core`
  contains no document types, which §2 below makes an invariant.
- **ADR 0007 rejected a separate `vecmanf-library-io`.** HTTP and credentials are
  two modules in `vecmanf-storage-io`, on KISS grounds: "split it out when a
  second provider family makes the crate's responsibility genuinely plural."
  This ADR follows that precedent for the relay client and the awareness channel
  rather than overriding it.
- **ADR 0009 §2's awareness channel is I/O, and the merge semantics it serves
  are not.** Ephemeral state (pointer, selection, hover, active tool, in-flight
  drag geometry) never enters the operation log, so its payload types belong
  with the state they describe — `vecmanf-ui-core` — and its transport belongs
  with the relay socket in `vecmanf-storage-io`.

### Options considered

**A. One flat workspace, one crate per responsibility as 0001–0010 named them.**
Chosen. Twelve Rust crates plus `frontend/`, one `Cargo.lock`, one `cargo deny`
run, profiles and lints at the root.

**B. Fewer, larger crates — fold document, geometry, render and UI logic into a
single `vecmanf-core`.** Rejected, and not on aesthetics: the one-way dependency
rule of `CLAUDE.md` §5 and ADR 0001 §1's "no UI framework type outside the
shell" are currently enforced *by the crate graph*, for free, by the compiler. In
one crate they become review conventions. It would also destroy ADR 0008 §5's
narrow server exception: with one core crate the server either depends on the
document model or reimplements the header parser, which is precisely the choice
0008 §5 was written to avoid.

**C. Separate repositories (or nested workspaces) for the server and the plugin
SDK.** Rejected. The server shares `vecmanf-crypto-core` with the client, so
splitting repos means publishing that crate or vendoring it — the drift 0008 §5
rejected, with release management added. One repository also discharges AGPL §13
for the hosted relay (ADR 0006, ADR 0008 §12) with no extra process.

**D. Extract the value types (units, `Path`, style, `Transform`) from
`vecmanf-document-core` into a small `vecmanf-model-core`, re-exported by
`vecmanf-document-core` at its historical paths** (the pattern in
`docs/guides/rust-workspace-blueprint.md` §1, which keeps the extraction
invisible to callers — `vecmanf_document_core::Length` keeps resolving, so
ADR 0002 §3 and ADR 0003 §1 stay true as written). It would keep Loro out of
four crates' dependency trees. **Rejected for now, with a named trigger** (§8):
the only argument that bites today is guest binary size for plugins, and plugins
are deferred past MVP. Loro is wasm-clean, so nothing is *broken* by its
presence — it is weight. And because of the re-export, D stays available later
for the cost of one `lib.rs`: doing it now is a speculative crate
(`CLAUDE.md` §5), doing it on measurement is cheap.

## Decision

1. **Flat workspace, option A.** Directory per crate at the repository root,
   plus `frontend/`:

   ```text
   vecmanf-document-core/   vecmanf-crypto-core/     vecmanf-editor-wasm/
   vecmanf-geometry-core/   vecmanf-library-core/    vecmanf-app/
   vecmanf-vectorize-core/  vecmanf-storage-io/      vecmanf-sync-server/
   vecmanf-render-core/     vecmanf-ui-core/         vecmanf-plugin/
   frontend/
   ```

   Root `Cargo.toml` carries `[workspace]` with `resolver = "2"`, the member
   list, `[workspace.package]` (`edition`, `rust-version`,
   `license = "AGPL-3.0-or-later"` per ADR 0006 §1), `[workspace.lints]` exactly
   as `CLAUDE.md` §5 lists them, `[workspace.dependencies]` for every shared
   external dependency so versions are pinned once, and `[profile.*]` — which
   Cargo reads only at the root. Crates version independently, so each gives its
   own explicit `version`.

2. **The crates and what each is responsible for** — one sentence each, and if a
   sentence needs "and" between two unlike things, that is a module split inside
   the crate, not a second crate:

   | Crate | Responsibility |
   |---|---|
   | `vecmanf-document-core` | The document model of ADR 0002 — units, nodes, paths, style, manufacturing intent, the command journal, the Loro backing, SVG import/export, and the `.vmf` container layout plus its format migrations as pure byte-level code. |
   | `vecmanf-geometry-core` | The geometric kernel of ADR 0003 — booleans, offsetting, simplification, V-carve depth — over document path types with an explicit `Tolerance` per call. |
   | `vecmanf-vectorize-core` | Raster-to-vector tracing (ADR 0003 §6), taking a decoded pixel buffer and settings, including the multi-pass combination of R-VEC-002. |
   | `vecmanf-render-core` | Document plus view transform to a flat draw list, tessellated with `lyon` at its own display tolerance (ADR 0001 §4, ADR 0003 §7). |
   | `vecmanf-ui-core` | Interaction logic as plain state and pure functions (ADR 0001 §1) — tool state machines, selection, hit-testing, snapping, command dispatch, undo driving — plus the awareness payload types of ADR 0009 §2. |
   | `vecmanf-library-core` | The record model of ADR 0004 §6 (machines, materials, test-result logs, asset catalogs, the font collection), its `merge(local, remote)`, search and filtering, and the asset-provider protocols as pure request/response functions (ADR 0007 §5, §14). |
   | `vecmanf-crypto-core` | The sealed envelope and the keyring as pure functions over bytes — seal, open, header parse, invite encode/decode, wrap/unwrap, entry hashing, signature verification, causal replay and fork merge (ADR 0008 §4, ADR 0010 §14) — with zeroizing key newtypes. |
   | `vecmanf-storage-io` | Every impure capability the desktop client needs, as one module each: filesystem and data directory, OS CSPRNG, clock, credential store, HTTP, git wire, relay socket. |
   | `vecmanf-editor-wasm` | The `wasm-bindgen` facade of ADR 0001 §3 — binding, (de)serialization, draw-list handoff and GPU submission, no logic of its own. |
   | `vecmanf-app` | The Tauri 2 host binary: platform I/O behind one narrow command interface (ADR 0001 §6), and the desktop `wasmtime` plugin host. |
   | `vecmanf-sync-server` | The blind relay and sealed-blob store of ADR 0004 §5 and ADR 0008 §6, with ADR 0010 §10's membership check and ADR 0004 §4's machine-resource leases. |
   | `vecmanf-plugin` | The guest-side Rust SDK of ADR 0005 §1: ergonomic wrappers over the `wit-bindgen` output. |

3. **Dependency direction, stated once and completely.** Every edge below points
   from a crate to a crate it may depend on; no edge runs the other way, and no
   edge not listed exists.

   ```text
   vecmanf-document-core   → (external only)
   vecmanf-crypto-core     → (external only)            ← invariant, see §4
   vecmanf-geometry-core   → document-core
   vecmanf-vectorize-core  → document-core, geometry-core
   vecmanf-render-core     → document-core
   vecmanf-ui-core         → document-core, geometry-core, vectorize-core
   vecmanf-library-core    → document-core
   vecmanf-plugin          → document-core
   vecmanf-storage-io      → document-core, library-core, crypto-core, ui-core
   vecmanf-editor-wasm     → document-core, ui-core, render-core
   vecmanf-app             → storage-io (and the core crates through it)
   vecmanf-sync-server     → crypto-core, and no other vecmanf crate
   ```

   Notes on the non-obvious edges. `geometry-core → document-core` is
   ADR 0003 §1's "takes and returns `vecmanf-document-core` path types", so
   document-core must never depend on geometry-core — which also settles who
   *executes* an operation command: not `document-core`, which cannot reach
   the kernel, but `vecmanf-ui-core`, which ADR 0001 §1 gives command dispatch
   and which therefore depends on both kernels it dispatches into
   (`geometry-core` for booleans, offsetting and hit-testing,
   `vectorize-core` for a trace). `library-core →
   document-core` exists only for the unit newtypes: a material thickness is a
   `Length` and a cut parameter a `Speed`/`Power`, and bare `f64` would violate
   `CLAUDE.md` §5 — this is the edge option D would delete. `storage-io →
   ui-core` carries nothing but ADR 0009 §2's awareness payloads to the relay
   socket. `render-core` and `ui-core` do not depend on each other: the view
   transform is the affine transform type from `document-core`, passed to both.

   **No `*-core` crate depends on `*-io`, on `*-app`, on `vecmanf-editor-wasm`
   or on the frontend, ever.** This is the `CLAUDE.md` §6 rule, and the crate
   graph is what enforces it.

4. **`vecmanf-crypto-core` depends on no other `vecmanf` crate, and that is an
   invariant rather than a current fact.** It is what keeps ADR 0008 §5's
   exception narrow: the server may depend on this one crate precisely because no
   document type can reach it, so a document-model change still cannot force a
   server deploy. A pull request adding any `vecmanf-*` dependency to
   `vecmanf-crypto-core`, or any second `vecmanf-*-core` dependency to
   `vecmanf-sync-server`, contradicts an accepted ADR and needs a new one. CI
   asserts both (§6).

5. **Where ADR 0001's three artefacts sit in the build.** `cargo` builds
   `vecmanf-editor-wasm` for `wasm32-unknown-unknown`; `wasm-bindgen` emits the
   JS glue; `frontend/` (Vite) consumes the module as a dependency and produces
   the bundle; `vecmanf-app` embeds that bundle at package time. So the Rust
   build never depends on the frontend and the frontend depends on exactly one
   Rust artefact. `frontend/` is not a workspace member, carries its own
   `package.json` (same SPDX identifier, ADR 0006 §1) and its own gate — `tsc
   --noEmit`, ESLint, Prettier, `vitest`, Playwright, plus an npm license and
   audit check, since `cargo deny` does not see npm (ADR 0001 consequences,
   ADR 0006 §2).

6. **Every `*-core` crate builds for `wasm32-unknown-unknown`, and the
   dependency tree is checked rather than assumed.** ADR 0003's own research is
   the proof that this fails silently: `centerline` was ruled out because it
   pulls `rayon`, which does not build for wasm32 without threads. So:
   - CI builds each of the seven `*-core` crates for `wasm32-unknown-unknown`
     in addition to the host, as `CLAUDE.md` §7 already requires.
   - CI additionally asserts, per core crate, that
     `cargo tree --target wasm32-unknown-unknown -p <crate> -i <dep>` finds
     nothing for a named ban list: `rayon`, `tokio`, `reqwest`, `keyring`,
     `gitoxide`, `mio`, `socket2`. A build failure says *that* wasm broke; this
     says *which* dependency did it.
   - Core crates declare external dependencies with `default-features = false`
     wherever the default pulls `std`-only, `getrandom` or thread paths. Two
     known cases: the RustCrypto signature crates default to `rand_core`, which
     must stay out because ADR 0008 §4 says `vecmanf-crypto-core` generates no
     randomness; and `uuid` generation is likewise *not* in
     `vecmanf-library-core` — records take their UUID as a parameter and
     `vecmanf-storage-io` mints it.
   - Two external dependencies are wasm-load-bearing and unverified: Loro, and
     whichever boolean crate `spike/booleans` picks. The spike already judges
     wasm build cleanliness (ADR 0003 §3); Loro's is evidenced by `loro-wasm`
     but confirmed by the first CI run, not by reputation.

7. **The CI matrix.** `vecmanf-app` and the crates it depends on run the host
   gate on ubuntu, windows and macos (`CLAUDE.md` §8). The wasm32 job and the
   `cargo tree` assertions of §6 run on ubuntu only. `vecmanf-sync-server` runs
   on ubuntu only — it is deployed there, and nothing in it is
   platform-specific. `vecmanf-plugin` and the ADR 0005 §1 example plugin build
   for the wasm component target and must keep compiling. `cargo deny` and
   `cargo doc` run once over the workspace.

   > **Note 2026-10-10 (CI schedule, customer decision 2026-10-09):** on pull
   > requests and pushes to main the host gate runs on ubuntu only. Windows
   > and macOS run it nightly (`schedule`, 02:30 UTC) and on
   > `workflow_dispatch`. Windows and macOS must not be required status checks
   > in branch protection: they never report on a pull request, so a required
   > check would block every merge. The host gate also runs over the whole
   > workspace, not only `curvyo-app`'s dependency closure. Details:
   > `.github/workflows/ci.yml` (header comment) and `docs/technical-debt.md`,
   > "Windows and macOS tests run nightly only". The crate layout of this ADR
   > is unchanged.

8. **No crate is created here that no accepted ADR named.** Three slots are
   deliberately left empty, so their absence reads as a decision rather than an
   oversight:
   - **A machine/toolpath core crate and a device `-io` crate.** `CLAUDE.md` §6
     layers job generation and output formats into core and device
     communication into platform crates, and gives each machine family its own
     ADR when its first story is ready. That ADR names those crates. ADR 0001
     §6's plural "`*-io` crates" anticipates the second one.
   - **`vecmanf-model-core`** (option D). Trigger: the plugin host story
     measuring a guest binary, or a third consumer needing units without the
     document. The extraction is a `lib.rs` re-export and a short ADR, and
     touches no consumer's source.
   - **A plugin-host core crate.** ADR 0005 §9 forbids a registry or capability
     enum before the host story. The desktop host is `wasmtime` in
     `vecmanf-app` and the browser host is the engine's own runtime behind
     `vecmanf-editor-wasm`; if that story finds a pure part worth sharing
     between them, it needs its own ADR.

## Consequences

- **The structural rules of `CLAUDE.md` §5/§6 become compiler-enforced rather
  than review-enforced.** A UI type cannot reach `vecmanf-document-core`, and
  the server cannot acquire the document model, because the graph does not
  permit it. That is the whole return on twelve crates instead of four.
- **Twelve crates and two ecosystems is a real amount of ceremony** for a project
  with no product code yet: twelve manifests, a longer `cargo build` graph, and
  a gate with host, wasm32, component and npm legs. Accepted, because every one
  of the twelve was authorized by a decision that needed it, and because the
  boundaries are cheapest to draw now.
- **`CLAUDE.md` §8's suffix list is incomplete and the lead must amend it.** It
  names `-core`, `-app` and `-io`; accepted ADRs also commit to
  `vecmanf-editor-wasm` (0001 §3), `vecmanf-sync-server` (0004 §5) and
  `vecmanf-plugin` (0005 §1). Renaming them would edit accepted ADR text, which
  the index forbids, so §8 gains the three categories instead. This folds into
  the §7/§8 amendment the index already lists.
- **`vecmanf-storage-io` is the crate most likely to need splitting later**, and
  it is deliberately not split now, following ADR 0007's own reasoning. It holds
  seven single-responsibility modules spanning filesystem, credentials, HTTP, git
  and a live socket. The named trigger is the one ADR 0007 gave: when a second
  provider family makes its responsibility genuinely plural. Recorded in
  `docs/technical-debt.md`.
- **The browser target has no `-io` crate and will not get one.** ADR 0001 §6's
  narrow command interface is implemented in TypeScript against OPFS, `fetch`
  and WebCrypto, so `vecmanf-storage-io` is desktop-only by design. The cost is
  that the browser's I/O layer is outside the Rust gate entirely and is covered
  only by the frontend's own.
- **`vecmanf-editor-wasm`'s facade will grow when the browser target gains
  collaboration**, which needs `vecmanf-crypto-core` and `vecmanf-library-core`
  behind it. That is an addition to the facade's contents, not a new crate, and
  ADR 0001 §3's "no logic of its own" still binds.
- **Every plugin guest currently carries Loro**, through
  `vecmanf-plugin → vecmanf-document-core`. This satisfies R-EXT-001's "built
  against the same core crates the application itself uses" literally, and
  option D is the escape hatch with its trigger named. Recorded in
  `docs/technical-debt.md`.
- **Nothing here is load-bearing on a customer decision**, so if the lead
  disagrees with any boundary it is a refactor under an ADR superseding this
  one, not a conversation with the customer.
