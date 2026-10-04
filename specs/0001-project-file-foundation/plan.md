# Plan for "Project file foundation: new, open, save a local project"

## Affected crates/modules

Creates the workspace root (ADR 0011 "first commit of product work") plus:

- **`vecmanf-document-core`** (new crate, `#![forbid(unsafe_code)]`,
  wasm32-clean): `Length`/`DocumentSize` newtypes (ADR 0002 §2/§3); a `Document`
  wrapping an internal `loro::LoroDoc` (never exposed in the public API, ADR
  0004 §3) holding a `root` map with `format_version` and `size` as per-field
  LWW registers (ADR 0009 §3); pure byte-level `.vmf` container pack/unpack
  (`manifest.json`, `document.loro`, `document.json`) over `&[u8]`/`Vec<u8>`,
  no `std::fs`; the `OpenError` taxonomy (`thiserror`, 3 variants: not a zip,
  damaged/incomplete, format too new) per adrs.md's feature-local decision.
- **`vecmanf-storage-io`** (new crate): filesystem module only for this slice —
  atomic write-temp-then-rename, read-to-bytes. No lock files (ADR 0004 §7).
  No credential/HTTP/relay/git modules yet — nothing in this slice needs them
  and `CLAUDE.md` §5 forbids speculative scope.
- **`vecmanf-app`** (new crate, Tauri 2 binary): native `Menu` (File: New,
  Open…, Save, Save As…, separator, Quit — accelerators per spec), native
  file dialogs (`.vmf` filter, `.vmf` file association), Tauri commands
  (`new_project`, `open_project`, `save_project`, `save_project_as`) that call
  into `vecmanf-document-core` + `vecmanf-storage-io`, and app-state holding
  the single open `Document` plus its current path. No device I/O, no updater
  plugin (adrs.md: "no network client of any kind in this build").
- **`frontend/`** (new, not a workspace member): Vite + TypeScript + React +
  Tailwind + shadcn/ui. Empty canvas div (`--canvas-bg`), status bar
  (`--statusbar-bg`, cursor mm left / size mm right), `AlertDialog` for AC7,
  title-bar sync via Tauri `window.setTitle`, listens for menu events and
  invokes the four commands.
- Root `Cargo.toml` (`[workspace]`, `[workspace.package]` incl.
  `license = "AGPL-3.0-or-later"`, `[workspace.lints]` per `CLAUDE.md` §5,
  `[workspace.dependencies]`), `rust-toolchain.toml`, `LICENSE`, `deny.toml`.

Not touched / not created: every other crate in ADR 0011's table (no story
needs them yet) — confirmed against adrs.md's "deliberately not in scope".

## Tasks

- [x] 1. Workspace scaffold: root `Cargo.toml`, `rust-toolchain.toml`,
      `LICENSE` (AGPL-3.0-or-later), `deny.toml` with ADR 0006 §2's allow-list.
      (infra for all ACs)
- [x] 2. `vecmanf-document-core`: `Length` newtype + `DocumentSize`
      (fulfils AC2's mm size).
- [x] 3. `vecmanf-document-core`: `Document::new()` — fresh Loro peer id
      (adrs.md feature-local decision), root map with `format_version` +
      `size` defaulting to 210×297mm. Unit tests. (AC2)
- [x] 4. `vecmanf-document-core`: container pack (`to_vmf_bytes`) writing
      `manifest.json` + `document.loro` (Loro `Snapshot` export) +
      `document.json` (plain export, read against a frozen version per
      ADR 0009 §4) into a zip, over `&[u8]`. Golden-file fixture: a valid
      `.vmf`. (AC3)
- [x] 5. `vecmanf-document-core`: container unpack (`Document::from_vmf_bytes`)
      — read `manifest.json` first and alone decide acceptance (feature-local
      decision), reject on missing/corrupt required members or
      `format_version` newer than supported, else reconstruct `Document` from
      `document.loro` only (never `document.json` on normal open). `OpenError`
      taxonomy. Golden-file fixtures: truncated/corrupt zip, future
      `format_version`, non-zip file. Round-trip test (pack → unpack →
      same size). (AC5, AC6, AC7)
- [x] 6. `vecmanf-storage-io`: `write_atomic(path, bytes)` (temp file in same
      dir + rename) and `read_to_vec(path)`. Unit tests with a tempdir.
      (AC3, AC4, AC5, AC6)
- [x] 7. `vecmanf-app`: Tauri scaffold, app state (`Mutex<Option<OpenProject>>`
      holding `Document` + path), commands `new_project`, `open_project`,
      `save_project`, `save_project_as` wired to document-core + storage-io;
      native menu with accelerators and `.vmf` file association; refusal path
      builds the replacement document fully before swapping state (AC7's
      "already-open project is left untouched"). (AC2–AC7, AC10 via shared
      codebase)
- [x] 8. `frontend/`: minimal Vite+React+TS+Tailwind+shadcn scaffold; canvas +
      status bar components; `AlertDialog` wired to the three error strings;
      title bar state machine ("vecmanf" / "<filename> — vecmanf"); menu-event
      listeners calling the four commands via `@tauri-apps/api`. (AC1, AC2,
      AC3, AC5, AC7)
- [x] 9. Wire `Save` (no re-prompt when a path is already known) vs
      `Save As` (always prompts, default filename `Untitled.vmf`). (AC4)
- [x] 10. Quality gate (`CLAUDE.md` §7): fmt, clippy, nextest, wasm32 build of
      `vecmanf-document-core`, `cargo deny`, `cargo doc`. Manual verification
      checklist for AC1, AC8, AC9, AC10 (see Validation) since they need a
      running app / network toggling / OS matrix that nextest cannot exercise.

## Validation

- **Golden-file tests** (`vecmanf-document-core/tests/fixtures/`): a valid
  `.vmf` (round-trips), a truncated/corrupt zip, a non-zip renamed-`.txt`
  file, a `manifest.json` with `format_version` one above current. Each
  pinned to the exact `OpenError` variant and user-facing-mapped string
  (AC7's three cases).
- **Unit tests in `vecmanf-document-core`**: default document size is
  210×297mm; peer id differs across two `Document::new()` calls; pack→unpack
  round trip preserves size; manifest is read and validated before
  `document.loro` is touched (a corrupt `document.loro` behind a valid
  manifest still produces `OpenError::Damaged`, not a panic).
- **Unit tests in `vecmanf-storage-io`**: atomic write leaves no temp file
  behind on success; a pre-existing file at the destination is replaced
  atomically (read back matches new bytes, never a partial write).
- **AC1 (3s launch), AC8 (offline), AC9 (zero outbound), AC10 (3-OS parity)**
  are not unit-testable from this worktree (need a packaged app, a network
  toggle, and Linux/Windows/macOS builds respectively). Flagged in the report
  to the lead/tester as manual-verification items: AC9 is checkable today by
  code inspection (no HTTP/updater/telemetry dependency anywhere in the new
  crates — confirmed by `cargo tree`), AC1/AC8/AC10 need the tester or a CI
  OS matrix this slice does not add (no `.github/workflows` exists yet; noted
  as an open point, not silently built).
- Full gate from `CLAUDE.md` §7 run before reporting done.
