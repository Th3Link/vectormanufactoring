# ADR 0013: Rename the product to Curvyo

**Status:** Proposed — needs-customer (architect, 2026-10-07)

The customer chose the name and registered `curvyo.org`: "since we are largely
through here, we rename to curvyo afterwards." This ADR fixes what the rename
touches, the decisions that are the customer's, and one mechanical procedure to
run it in a single quiet window. It amends [ADR 0011](0011-workspace-and-crate-layout.md)
(crate and directory names) and the file-extension line of
[ADR 0004](0004-persistence-and-cross-machine-sync.md) §1. Accepted ADR text is
not edited (index rule), so this ADR is the name map. The full inventory with
counts and per-category risk is in
[`0013-rename-to-curvyo-inventory.md`](0013-rename-to-curvyo-inventory.md).

It is `needs-customer` because it changes a file extension that users will
see and a bundle identifier that cannot change cheaply after release
(`CLAUDE.md` §3: removing or changing accepted user-facing behaviour). It does
not touch the document model, the container layout or any wasm/core boundary.

## Context

What the old names are, measured on `main` at `48ecd76`:

- **Seven crates** under `vecmanf-*/` (171 tracked files), 250 `use vecmanf_*`
  lines in 124 `.rs` files, 780 `.rs` lines mentioning the name in all, plus
  16 lines of `Cargo.lock`, 7 `deny.toml` licence exceptions and the workspace
  member list. The wasm module stem `vecmanf_editor_wasm` is baked into
  `frontend/scripts/build-wasm.sh` and one TypeScript import.
- **The extension `.vmf`**: 178 lines in 49 files, the Tauri file association,
  the open/save dialog filter, the OS-open path check, `OpenError::NotAVmf`,
  user-visible error texts, and eight golden fixtures
  (`vecmanf-document-core/tests/fixtures/*.vmf`).
- **Tauri**: `productName: "vecmanf"`, `identifier: "dev.vecmanf.app"`, window
  title, file association name. No updater, no deep-link scheme.
- **Docs**: ADRs 0001–0012 (131 lines, 74 of them in 0011), `technical-debt.md`,
  15 spec folders (323 occurrences; no folder name contains the old name).
- **Not present**, so nothing to migrate: any `vecmanf`/`vmf` string *inside* a
  saved container (`manifest.json` holds only `format_version`,
  `loro_snapshot_version`, `app_version`; the Loro snapshot and `document.json`
  contain neither string, checked in all eight fixtures); `localStorage`,
  IndexedDB, keychain entries, config or data directories written by the app;
  environment variables; cargo features; licence headers (ADR 0006 §1 requires
  none; `LICENSE` is the stock AGPL text); the domain; Renovate config.

The format has no magic string or mimetype: `unpack` recognises a project by its
zip signature and a valid `manifest.json`, never by extension. That is what
makes the extension decision cheap.

## Decisions

Each has a default. Silence from the customer means the default is taken.

### D1. Crate and directory prefix

- **A. `curvyo-*`, same suffix scheme (`-core`, `-app`, `-io`, `-wasm`,
  `-server`, suffix-less `curvyo-plugin`)** (default). `curvyo-app` is fine: it
  is the Rust package and raw binary name; the installed app is named by Tauri's
  `productName`, not by the package.
- B. Drop the prefix (`document-core`, ...). Rejected: generic names collide in
  the dependency graph and in `cargo doc`, and ADR 0011's "no other `curvyo`
  crate" rules (§4) lose their handle.
- C. Keep `vecmanf-*` internally, rename only the product. Cheapest (no Rust
  churn) and rejected: every `use`, every CI job name and every error message
  keeps a name the product no longer has, forever.

The six planned crates not yet created (`curvyo-vectorize-core`,
`-library-core`, `-crypto-core`, `-sync-server`, `-plugin`, `-model-core`) take
the new prefix from the start.

### D2. File extension and in-file identifiers

There is no in-file identifier to change, so no `format_version` bump and no
migration code. Only the extension is at stake.

- **A. Rename to `.curvyo`** (default, recommended). Pros: nothing is released,
  so this is the last free moment; the extension matches the product and is
  unambiguous in file managers and in OS association tables; fixtures are
  `git mv` only (bytes unchanged). Con: the customer's own saved `*.vmf` test
  projects no longer show in the open dialog (see below). Seven characters.
- B. Keep `.vmf` for now. Pros: zero fixture and association churn; own test
  files keep working. Con: the abbreviation stops meaning anything, and
  changing it after the first release means shipping two extensions for good.
- C. A short new extension (`.cvy`). Not recommended: shorter but obscure, and
  I cannot check registries for collisions from here.

**Existing files.** `.vmf` files open unchanged once renamed; their bytes are
valid as they are. For the customer's own projects:
`for f in *.vmf; do mv "$f" "${f%.vmf}.curvyo"; done`. No converter. If the
customer would rather not rename, the optional alias is two lines in
`curvyo-app/src/main.rs` (dialog filter `&["curvyo", "vmf"]`, and accept both in
the two OS-open path checks); save still writes `.curvyo`. Default: no alias,
because the only existing files are the customer's own test projects and an
alias that is never removed is the "two extensions for good" cost of option B.
It is the same kind of read alias as the `"smooth"` tag migration in
`vecmanf-document-core/src/document.rs`, except that here it is a dialog filter,
not stored data.

Identifier names do not follow the extension: `VMF_EXTENSION` becomes
`PROJECT_EXTENSION`, `NotAVmf` becomes `NotAProject`, so a later extension
change touches one constant.

### D3. Tauri

- `productName` `"Curvyo"`; window title `"Curvyo"`; file association name
  `"Curvyo project"`, description `"Curvyo project (*.curvyo)"`.
- **Identifier: `org.curvyo.desktop`** (default) rather than `org.curvyo.app`.
  The current `dev.vecmanf.app` already ends in `.app`, which Tauri's bundler
  warns about because it clashes with the macOS bundle extension (from the
  Tauri docs as I recall them; the implementer confirms with one
  `cargo tauri build` and uses `org.curvyo.app` if it does not warn). Reverse
  domain of the registered `curvyo.org`, so nothing else is registered under it.
- **What the identifier change does.** Tauri keys the per-app directories
  (data, config, cache, log) and, per platform, the webview's storage by the
  identifier, so the app starts with fresh ones. Today that loses nothing: the
  app writes no files of its own and the frontend uses no `localStorage` or
  IndexedDB; only webview cache is rebuilt. The old `dev.vecmanf.app`
  directories can be deleted by the customer. This is the cheap moment: ADR 0004
  §12 (data directory, library, fonts) and ADR 0007 (keychain entries) will key
  off the identifier, and after release it cannot change without a migration.
  There is no updater and no deep-link scheme to coordinate.

### D4. In-app strings and casing

Display name **Curvyo** (capital C) in window title, error dialogs
(`open_error.rs`, four texts), HTML title, dialog filter and prose. Lower-case
`curvyo` in identifiers, crate names, paths and GPU debug labels. The 53 bare
occurrences of the word are reviewed by hand, not by script.

### D5. Accepted ADRs and history

- ADR 0001–0012 bodies stay as written. Their Status lines for 0011 and 0004
  gain "names amended by ADR 0013"; the index gets a one-line pointer: read
  `vecmanf-*` as `curvyo-*` and `.vmf` as `.curvyo` in 0001–0012.
- Specs, `technical-debt.md`, `design-system.md`, `README.md`, `CLAUDE.md`,
  agent files and code comments are rewritten mechanically: they are living
  documents, and a stale name there misleads.
- Four PR links in `specs/000{1..4}-*/specification.md` point at
  `Th3Link/vectormanufactoring` and are left alone: GitHub redirects them.
- After this ADR, no new persistent identifier (data directory, keychain
  service, HKDF context string, relay protocol tag, git ref namespace) may
  contain the old name.

### D6. GitHub repository

Rename `Th3Link/vectormanufactoring` to **`Th3Link/curvyo`** (default), done by
the customer in GitHub settings *after* the rename PR is merged. Web, `git` and
API redirects keep old URLs working until a new repo takes the old name.
Afterwards: `git remote set-url origin https://github.com/Th3Link/curvyo.git`
in `base` (worktrees share it); the first CI run has cold caches; if branch
protection requires status checks by name, update them (the matrix job
`core-wasm32 (<crate>)` is named after the crates, so those names change); if
Renovate runs as the GitHub app it follows the rename, otherwise point it at
the new name (the repo holds no Renovate config and no CI badge).

### D7. Local working directory and Claude memory

**Keep `/home/marc/workbench/vecmanf-claude`** (default and recommendation). It
is the customer's folder, not part of the product. Sessions, history and the
auto-memory index are keyed by that exact path under `~/.claude/projects/`;
moving it makes memory and session history appear empty. Twenty-two lines in
nine tracked files (`CLAUDE.md` §4, `.claude/agents/implementer.md`, four spec
plans, three `.projectatlas/*.json`) hold the path and stay valid. If the
customer wants it renamed anyway, at the very end, with no session running:
remove every worktree but `base` (`git worktree remove`); `mv` the folder; `mv`
the matching `~/.claude/projects/<old-path-slug>` directories (the folder slug
and the `-base` slug) to the new slugs; replace the path in the nine files;
rebuild `.projectatlas` (config paths, tracked `projectatlas.db`); run
`git worktree repair`; re-check any permission rule that names the path. The
architect does not touch anything under `~/.claude`.

### D8. Domain

`curvyo.org` appears only as the bundle identifier. No docs URLs, contact
addresses or placeholder site: out of scope until a story asks for them.

### D9. Timing

One window, after #47 (shape-creation-from-center) and polygon-star-box-refit
are merged, with no other branch or worktree open (`git worktree list` shows
only `base`) and `main` frozen until the verification below passes. Any open
branch conflicts with ~800 changed Rust lines and every renamed path.

## Procedure

Owner: **one implementer** on `chore/rename-to-curvyo` in
`/home/marc/workbench/vecmanf-claude/rename` (no sub-delegation). `chore/` PRs
merge without asking (`CLAUDE.md` §9); the lead still starts the window only
after the customer's go on D1–D7.

1. **Baseline on `main`.** Full gate green. Record: `git ls-files | wc -l`
   (525), `cargo nextest list | wc -l`, `git grep -c 'use vecmanf_'` sum (250
   lines), `sha256sum` of the eight fixtures.
2. **Move paths** (`git mv`, so history follows):
   ```sh
   for c in document-core geometry-core ui-core render-core editor-wasm storage-io app; do
     git mv vecmanf-$c curvyo-$c; done
   cd curvyo-document-core/tests/fixtures
   for f in *.vmf; do git mv "$f" "${f%.vmf}.curvyo"; done
   git mv not_a_vmf.txt not_a_project.txt
   ```
   Then `rm -rf vecmanf-app frontend/src/wasm-bindings frontend/dist` (ignored
   leftovers: `gen/`, stale bindings under the old stem).
3. **Rewrite text** with `perl -pi` over `git ls-files` minus: binary files,
   `.claude/skills/`, `.agents/`, `.projectatlas/projectatlas.db`,
   `docs/adr/00{01..12}-*.md`, `docs/adr/0013-*`. Rules, in this order:
   1. protect `vecmanf-claude`, `vectormanufactoring`, `Th3Link` (negative
      lookahead);
   2. `dev\.vecmanf\.app` to `org.curvyo.desktop`;
   3. `VMF_` to `PROJECT_`, `Vmf\b` to `Project`, `_vmf` / `vmf_` to
      `_project` / `project_`, `\.vmf\b` to `.curvyo`;
   4. `vecmanf_` to `curvyo_`, `vecmanf-` to `curvyo-`;
   5. remaining bare `vecmanf` and `"vmf"`: by hand (UI strings "Curvyo").
4. **Hand edits:** `tauri.conf.json` (D3), `main.rs` (title format
   `{name} — Curvyo`, extension constant `"curvyo"`, filter name),
   `open_error.rs`, `frontend/index.html`, `.github/workflows/ci.yml` (the
   `discover-core-crates` filter `. == "curvyo-editor-wasm"`, the only
   functional line of 16), `deny.toml`, `.gitignore`, `README.md` title,
   `CLAUDE.md` §8, ADR Status lines of 0011 and 0004 and the index pointer.
5. **Lockfile:** `cargo metadata --offline >/dev/null`, then
   `git diff --stat Cargo.lock` must show only workspace-member name and
   dependency lines (16), no third-party version change; `cargo metadata
   --locked` must pass.
6. **Regenerate** `.projectatlas` (names inside the tracked database).
7. **Grep gate:** `git grep -iE 'vecmanf|\bvmf\b|\.vmf'` returns only the
   allow-list: ADR 0001–0012 bodies, this ADR and its inventory, the 22 path
   lines (`vecmanf-claude`), nothing else. `git grep -c 'use curvyo_'` sums to
   250. `git diff -M --summary` shows the eight fixtures as 100% renames.
8. **One commit** for the whole mechanical change (a second only for hand
   edits, to ease review), PR description lists D1–D9 as accepted.

## Verification (tester, independent of the implementer)

- Full gate: `cargo fmt --all --check`; clippy `-D warnings`; nextest with the
  same test count as the baseline; `cargo build --target
  wasm32-unknown-unknown -p` for each `curvyo-*-core` and `curvyo-editor-wasm`;
  `cargo deny check`; `cargo doc` with `-D warnings`.
- Frontend: `npm ci`, `npm run build:wasm` (emits `curvyo_editor_wasm.js`),
  `npx tsc -b --noEmit`, `npm run lint`, `npm run build`.
- Fixtures: all eight open or are refused exactly as before (the existing
  golden tests cover it; the baseline hashes match).
- App: `cargo tauri dev` shows "Curvyo"; File > Open lists `*.curvyo`; Save
  defaults to `Untitled.curvyo`; a renamed old test project opens; error text
  names Curvyo. One `cargo tauri build` for the identifier warning and the
  bundle names.
- CI green on the PR on all three OS legs, `core-wasm32 (curvyo-...)` jobs
  included.

## Rollback

Before merge: close the PR, delete the branch, `git worktree remove`. After
merge: `git revert` of the single squash commit on `chore/revert-rename`;
clean only while `main` is frozen, which is why D9 freezes it. The GitHub
repo rename is reversible by renaming back. The extension and the identifier
have not shipped, so a revert strands no user data; the customer's renamed
test files keep opening under either name (content, not extension, identifies
a project).

## Who does what

| Who | Does |
|---|---|
| Customer | Answers D1–D7; says "go"; renames the GitHub repo after merge; optional: renames his `*.vmf` projects, removes `dev.vecmanf.app` data directories, local folder (D7) |
| Lead | Confirms no open branch or worktree; dispatches one implementer; opens the PR; starts the window only after customer go; updates remotes and branch-protection checks after the repo rename |
| Implementer | Steps 1–8, one branch |
| Tester | Verification list, PASS/FAIL, grep allow-list check |
| Architect | Reviews the PR (crate names, ADR Status lines, index); accepts this ADR on the customer's answers |

## Consequences

- One loud, reviewable PR; no compatibility shims survive it.
- ADR 0001–0012 keep the old names permanently; readers use the name map above.
- The bundle identifier and extension are fixed before release, at the cost of
  one manual `mv` for the customer's own test files.
- Residual old-name strings after the window, deliberately: the working
  directory path (22 lines), four historical PR URLs, ADR 0001–0012 bodies.
