# ADRs for "Project file foundation: new, open, save a local project"

This slice is the first thing to exercise the `.vmf` container and the Loro
backing already decided in ADR 0004 (`specs/index.md`, "Notes on ordering"). It
reopens none of those decisions.

## Depends on

- [ADR 0001](../../docs/adr/0001-ui-framework-and-canvas-rendering.md): the app
  is a Tauri 2 host with a TypeScript/React/Tailwind + shadcn/ui frontend; the
  host (`vecmanf-app`) owns file dialogs, the file association and all
  filesystem access behind one narrow command interface (§6), and no UI type
  reaches a `*-core` crate (§1) — so "Save As" is a host command, never a core
  call.
- [ADR 0002](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  defines what an empty document *is* and therefore what `document.json`
  contains — millimetre as the canonical unit stored as `f64` (§2), unit
  newtypes rather than bare `f64` for the status bar's `x`/`y` and size (§3),
  Y-down document space (§4), a node tree with CRDT-minted `NodeId`s (§5, empty
  here: no nodes), and `format_version` from the first commit (§11). §9 also
  settles that the command journal is **not** persisted — this slice writes no
  journal.
- [ADR 0004 §1](../../docs/adr/0004-persistence-and-cross-machine-sync.md): the
  `.vmf` file format itself — a zip container holding `document.loro` (the
  authoritative CRDT snapshot), `document.json` (a plain, non-authoritative
  export written on every save and **never read on a normal open**), and
  optional `assets/`, `jobs/`, `thumbnail.png`, `keyring.log`. This is what
  AC3 checks and the reason AC5/AC6 restore from `document.loro` alone.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  format versioning from the first commit — readers accept their own version and
  older, writers always write current, **a newer file is refused with a clear
  message and never partially read**. That is AC7's third case verbatim, and it
  covers the Loro snapshot version as well as the container.
- [ADR 0004 §2, §7](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  an open document is a Loro replica, so AC6's full-restart round trip is
  snapshot export/import, not a bespoke serializer. §7's write obligations apply
  from the first save even on the local-only path: atomic
  write-temp-then-rename, a configurable data directory, and no lock files.
- [ADR 0009 §4](../../docs/adr/0009-concurrent-editing-semantics.md): every read
  that produces output runs against an immutable version snapshot. The
  `document.json` export is such a read, so a single save writes both members
  from one frozen version and the two can never describe different states.
- [ADR 0011 §2, §3](../../docs/adr/0011-workspace-and-crate-layout.md): which
  crates this slice touches and in which direction —
  `vecmanf-document-core` (the `.vmf` container layout and its migrations as
  pure, byte-level, wasm-clean code: zip in/out over `&[u8]`, no `std::fs`),
  `vecmanf-storage-io` (filesystem module: paths, atomic writes),
  `vecmanf-app` (Tauri host, native menu, native dialogs, file association) and
  `frontend/` (canvas element, status bar, `AlertDialog`). It also creates the
  workspace root that ADR 0011 describes but does not create (ADR index, "Open
  follow-ups").
- [ADR 0006 §1](../../docs/adr/0006-license.md): the first manifests written
  here carry `license = "AGPL-3.0-or-later"`, and `frontend/package.json` the
  same SPDX identifier.

## Deliberately not in scope for this slice

- [ADR 0008](../../docs/adr/0008-end-to-end-encryption-of-sync-and-collaboration.md)
  (E2EE) and
  [ADR 0010](../../docs/adr/0010-document-keyring-admins-and-revocation.md)
  (keyring, admins, revocation) are **not** applied here, and that is a
  decision, not an omission. ADR 0008 §7 is explicit: local `.vmf` files are
  **plaintext at rest by decision** — encryption is a sync and relay transport
  concern, and encrypting a local file with a key on the same disk is theatre.
  Sealing happens at the upload boundary (ADR 0004 §1, ADR 0008 §9), which this
  slice does not have. Nothing cryptographic is therefore missing from a
  conforming `.vmf` written here.
- Where they plug in later, so the shape stays additive: a shared document gains
  `keyring.log` as one more container member beside `document.loro`
  (ADR 0004 §1 — it sits outside the snapshot precisely so a joiner can read
  their own key wrap first); sealing wraps the bytes on the way to the relay or
  the blob store, not the bytes on local disk; and device identity keys
  (ADR 0010 §1) are generated when the first sharing story needs them. None of
  it changes a file this slice writes.
- `vecmanf-crypto-core`, `vecmanf-sync-server`, the relay socket module of
  `vecmanf-storage-io`, and any credential store (ADR 0007 §1) get no code here.
- No GPU canvas: nothing is drawn, so ADR 0001 §4's `wgpu`/WebGL2 path,
  `vecmanf-render-core` and `vecmanf-editor-wasm` are not required by any
  acceptance criterion and should not be stood up merely to clear a background
  colour (`CLAUDE.md` §5). ADR 0001's obligation to **measure WebKitGTK canvas
  performance with a real stress scene before the first canvas story** is still
  owed, and comes due with `path-node-editing`, not here.

## Feature-local decisions

- **2026-10-02: container version lives in a `manifest.json` member** —
  extends ADR 0004 §1's member list, additively. AC7 must refuse a too-new file
  *before* trusting its contents, and the two places the version could otherwise
  live both fail: `document.json` is "never read on a normal open" (§1) and is
  non-authoritative anyway, and `document.loro` cannot be parsed to learn that
  its own snapshot version is unsupported. So the container carries
  `manifest.json` with `format_version` (container), `loro_snapshot_version` and
  an informational `app_version`; it is read first and alone decides acceptance.
  Rejected: the zip archive comment (not self-describing, hostile to ordinary
  zip tooling) and a version field in `document.json` (gating a normal open on
  the non-authoritative export contradicts §1's "exactly one source of truth").
- **2026-10-02: required versus optional members on read.** Required to open:
  `manifest.json` and `document.loro`. Required on write: those two plus
  `document.json` (ADR 0004 §1's "written on every save", and AC3). Everything
  else — `assets/`, `jobs/`, `thumbnail.png`, `keyring.log` — is optional, and a
  reader must treat absence as normal rather than as damage; this slice writes
  none of them. In particular **no `thumbnail.png` is written yet**: its only
  consumers (recent projects, a browse surface) are out of scope, and an empty
  canvas's thumbnail would be a blank rectangle.
- **2026-10-02: no network client of any kind in this build** — AC9's
  zero-outbound requirement makes the Tauri updater plugin, any remote frontend
  asset (web fonts, CDN scripts, telemetry) and any launch-time version check a
  defect rather than a feature. ADR 0001 §6 does authorize auto-update in
  `vecmanf-app`; when it arrives it must be an explicit user-triggered check or
  an opt-in, never a launch-time poll, or it breaks this criterion retroactively.
  All frontend assets are bundled.
- **2026-10-02: a minimal document root record, and no more.** AC2 needs a
  document size in millimetres, and no accepted ADR defines one (ADR 0002 §5
  describes the node tree; R-MFG-001's work area belongs to a machine profile,
  not a document). The root therefore carries exactly `format_version` and
  `size: (Length, Length)`, defaulting to 210×297 mm, each a per-field
  last-writer-wins register per ADR 0009 §3. Page setup, orientation, margins,
  bleed and any binding to a machine work area are *not* added here — they
  belong to `machine-profile` (slice 10), which may need an ADR extending
  ADR 0002 to introduce them properly. Flagged to the lead: this is the one
  decision in this slice that touches protected ground (the document model), and
  it is kept to the two fields AC2 actually requires.
- **2026-10-02: open-refusal reasons are a typed error in
  `vecmanf-document-core`** (`thiserror`, three variants — not a zip, damaged or
  incomplete, version too new) that the host maps and the frontend renders as
  the three sentences in the UX notes. Keeps AC7's taxonomy a pure function of
  bytes, testable with fixture files and no UI, and keeps the mapping table out
  of TypeScript. The open path builds the new document fully before it replaces
  anything, so a refusal cannot touch an already-open project (AC7).
- **2026-10-02: a fresh Loro peer id per open session.** No device identity
  exists yet (ADR 0010 §1 is out of scope), nothing in this slice depends on
  peer-id stability across sessions, and undo is peer-scoped but there is no
  dirty state to undo (`specification.md`, Out of scope). Do not invent a
  persisted peer id in a config file or derive one from the hostname: when device
  identity lands it is the thing the peer id should be tied to.
  **Amended 2026-10-03 (architect, after review):** "fresh per session" must not
  mean *minted inside the core crate*. `LoroDoc::new()` draws its peer id from
  `getrandom`, so a `*-core` crate reaching an entropy source is the same
  violation ADR 0011 §6 already rules out for `uuid` in `vecmanf-library-core`
  ("records take their UUID as a parameter and `vecmanf-storage-io` mints it").
  The peer id is a parameter of `Document::new`, and `vecmanf-app` mints it —
  from `vecmanf-storage-io`'s CSPRNG module once that exists, which is also what
  makes the document model deterministic under test.

## Architect review notes (2026-10-03)

Recorded here rather than as edits to accepted ADR text, per the ADR index's
convention.

- **`vecmanf-document-core` needs a JavaScript host on `wasm32`, and a clean
  `wasm32-unknown-unknown` build does not prove otherwise.** Verified by hand on
  this branch: `loro` → `loro-internal` → `getrandom 0.2` with the `js` feature
  on, pulling `js-sys` and `wasm-bindgen` into the crate's wasm dependency tree.
  Declaring `loro` with `default-features = false` — which this slice does — does
  not prevent it. The browser target is unaffected (ADR 0001's engine is a JS
  host), but **a non-JS wasm host cannot instantiate this crate**, and ADR 0005's
  `wasmtime` plugin guests are exactly that, reached through
  `vecmanf-plugin → vecmanf-document-core` (ADR 0011 consequences, "Every plugin
  guest currently carries Loro"). This is a second and sharper trigger for
  ADR 0011 §8's `vecmanf-model-core` extraction (option D), independent of guest
  binary size. ADR 0011 §6's named ban list (`rayon`, `tokio`, …) cannot catch
  this class: the build *passes*. CI must additionally assert, per core crate,
  that `getrandom`/`js-sys`/`wasm-bindgen` appear in the
  `--target wasm32-unknown-unknown` tree only where a decision says they may.
- **`BSL-1.0` added to `deny.toml`'s allow-list, extending ADR 0006 §2's list.**
  One crate: `xxhash-rust` 0.8.19, transitive via `loro`. Boost Software
  License 1.0 is OSI-approved, FSF-free, strictly more permissive than MIT and
  GPL/AGPL-compatible, so it clears ADR 0006 §2's bar. Verified: it is the only
  BSL crate in the graph, and the graph carries no CDDL/SSPL/BUSL or
  source-available license. This is the "decision, not a waiver" §2 asks for.
- **Four RustSec advisories acknowledged in `deny.toml`, all verified.**
  `im` (RUSTSEC-2026-0248), `sized-chunks` (RUSTSEC-2026-0251) and `bitmaps`
  (RUSTSEC-2026-0247) via `loro-internal`; `proc-macro-error`
  (RUSTSEC-2024-0370) via Tauri's Linux `glib-macros` build dependency. Each is
  `informational = "unmaintained"` with `patched = []` in the advisory database —
  no security vulnerability and no upgrade we control. `cargo deny check` passes
  with these and without any other suppression.
- **`oxlint` replaces the ESLint named in ADR 0011 §5.** Accepted: same job,
  same gate slot, and §5's obligation is that the frontend *has* a lint leg, not
  which binary runs it. Prettier, `vitest`, Playwright and the npm license/audit
  check from §5 are still missing — that is the existing
  `docs/technical-debt.md` entry "The quality gate covers only half the
  product", now describing real code rather than a hypothetical.
- **`vecmanf-app` depends on `vecmanf-document-core` directly**, not only
  through `vecmanf-storage-io` as ADR 0011 §3's edge list reads
  ("`vecmanf-app → storage-io (and the core crates through it)`"). Accepted and
  correct: `vecmanf-storage-io` moves bytes and knows nothing about documents, so
  routing document types through it would make it a re-export shim. §3's
  parenthetical is read as "and the core crates", not "only via storage-io". No
  direction is inverted and no `*-core` crate gains an outward edge, so this
  needs no superseding ADR — §4's "new ADR required" rule binds
  `vecmanf-crypto-core` and `vecmanf-sync-server` only.
