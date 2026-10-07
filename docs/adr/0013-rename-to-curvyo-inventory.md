# Appendix to ADR 0013: inventory of the old names

Companion to [ADR 0013](0013-rename-to-curvyo.md). Measured on `main` at
`48ecd76` with `git grep` over tracked files (525 files; binaries and the
third-party `.claude/skills/` and `.agents/` trees show no hits). "Lines" are
lines containing at least one match. Re-run the same greps at the start of the
window: counts drift while PRs land.

Whole repository: 213 files and 1410 occurrences of `vecmanf` (lower case; the
capitalised forms do not occur), 50 files and 225 occurrences of `vmf`, 4 of
`vectormanufactoring`, 4 of `Th3Link`. 53 of the `vecmanf` occurrences are the
bare word (prose, UI strings, labels), the rest are `vecmanf-` (739) or
`vecmanf_` (618) compound names.

## 1. Inventory by category

| # | Category | Where | Count | Risk |
|---|---|---|---|---|
| 1 | Crate directories | `vecmanf-{document,geometry,ui,render}-core`, `-editor-wasm`, `-storage-io`, `-app` | 7 dirs, 171 tracked files (36, 4, 50, 16, 47, 5, 13 for document, geometry, ui, render, editor-wasm, storage, app) | `git mv` keeps history. Any open branch conflicts on every path. Ignored leftovers (`vecmanf-app/gen/`) stay behind and must be deleted |
| 2 | Cargo manifests | root `Cargo.toml` (7 member lines), 7 crate manifests (23 lines: `name`, `description`, path deps with `version`) | 30 lines | Low. Wrong edit fails `cargo metadata` at once |
| 3 | `Cargo.lock` | workspace-member `[[package]]` entries and their dependency lines | 16 lines | An uncommitted lock change shows up as a dirty tree and breaks any `--locked` build. The diff must show no third-party version change |
| 4 | Rust paths | `use vecmanf_*` and in-code paths: `vecmanf_document_core` 440, `_ui_core` 92, `_render_core` 37, `_editor_wasm` 29, `_geometry_core` 8, `_storage_io` 4 | 610 occurrences, 250 `use` lines, 124 files; 780 lines in 138 `.rs` files mention the name in any form | Mechanical and compiler-checked. Rustdoc intra-doc links break only under `-D warnings` doc build, covered by the gate |
| 5 | Rust strings and labels | GPU debug labels `"vecmanf ..."` (10 in `gpu.rs`), `expect("error while building the vecmanf application")`, `unwrap_or("vecmanf-project")` default file stem in `storage-io` | about 13 | Cosmetic, but the `storage-io` default stem and the window title are user-visible |
| 6 | wasm and JS bindings | `frontend/scripts/build-wasm.sh` (package `-p`, `.wasm` path, `--out-name vecmanf_editor_wasm`: 3 lines); `frontend/src/lib/editorSession.ts` imports `@/wasm-bindings/vecmanf_editor_wasm.js` (2 lines) | 5 lines | Stale generated output under the old stem in `frontend/src/wasm-bindings/` (ignored) is not removed by `git mv`; delete it. CI builds it fresh. Exported class and function names (`WasmSession`, ...) do not contain the name |
| 7 | Frontend, other | 14 files, 32 occurrences: comments naming crates (`Canvas.tsx`, `useEditorSession.ts`, `cursors.ts`, ...), `index.html` `<title>vecmanf</title>`, `frontend/README.md` (4), `vite.config.ts` comment, `.vmf` in 6 comments/docstrings. `package.json` name is `frontend` (private), `components.json` and `package-lock.json` hold no match | 32 | Only the HTML title is user-visible. No `localStorage`, `sessionStorage` or IndexedDB use anywhere, so no browser-side data to lose |
| 8 | Tauri config (`curvyo-app/tauri.conf.json`) | `productName` `"vecmanf"`, `identifier` `"dev.vecmanf.app"`, window `title` `"vecmanf"`, `fileAssociations` `ext ["vmf"]`, `name "vecmanf project"`, `description "vecmanf project (*.vmf)"`. `bundle` has no publisher, copyright, category or updater block; no updater plugin in `Cargo.lock` (`tauri-plugin-dialog`, `-fs` only); no deep-link scheme | 6 values | **Identifier** is the high-risk one: it keys the per-app data, config, cache and log directories and webview storage, so changing it starts the app with fresh ones (nothing is stored today). After release it cannot change cheaply. The `.app` suffix in the current identifier clashes with the macOS bundle extension (Tauri warns; confirm once). Linux package and `.desktop` names and the macOS and Windows file association registration follow `productName` and the extension: a previously installed `vecmanf` build stays a separate app |
| 9 | App code strings | `curvyo-app/src/main.rs`: `VMF_EXTENSION "vmf"`, `VMF_FILTER_NAME "vecmanf project"`, `DEFAULT_FILE_NAME "Untitled.vmf"`, title format `"{name} — vecmanf"` and bare `"vecmanf"`, two OS-open extension checks; `open_error.rs`: four user-visible texts ("This file isn't a vecmanf project (.vmf) file.", "...saved by a newer version of vecmanf...") | about 15 lines | User-visible, tested by existing acceptance tests (they assert the texts): sed must hit code and tests together |
| 10 | CI (`.github/workflows/ci.yml`) | 16 lines, 15 of them comments; one functional: `discover-core-crates` selects `endswith("-core") or . == "vecmanf-editor-wasm"`. Job names `core-wasm32 (${{ matrix.crate }})` are built from crate names. Artifact `frontend-dist`, job ids and paths contain no old name (paths are `frontend/...`, the workspace is built with `--workspace`) | 16 lines | Missing the `discover` filter silently drops `curvyo-editor-wasm` from the wasm32 matrix (a green CI that checks less). Required status checks in branch protection that name `core-wasm32 (vecmanf-...)` would never report: update after merge. The workflow trigger names (`CI`) do not change |
| 11 | `deny.toml` | 7 `[[licenses.exceptions]]` entries, `name = "vecmanf-..."` | 7 | A missed entry makes `cargo deny` fail on the AGPL licence of that crate: caught by the gate |
| 12 | `.claude/` | `agents/implementer.md` 3 lines (all the working-directory path); `agents/{architect,product-owner,tester,ux-engineer}.md`, `settings.json`, `hooks/*.sh`, `launch.json` contain no match; skills contain none | 3 | Path, not name: unchanged if D7 keeps the directory |
| 13 | `CLAUDE.md` | §4 working-directory rule (4 lines, path), §8 `vecmanf-plugin` (1 line) | 5 | Loaded into every session: a half-renamed file confuses agents. Rewrite §8 only; leave the path |
| 14 | README and `frontend/README.md` | `# vecmanf` title (1), `# vecmanf frontend` plus crate names (4). Separately: README "Status" still says pre-code and "ten accepted ADRs", which is stale (not part of the rename; product-owner to refresh) | 5 | Low. No badges, no URLs to the repo |
| 15 | ADRs 0001–0012 | 131 lines: 0011 74, 0004 12, 0008 10, 0010 9, 0007 9, 0001 8, 0003 6, 0002 1, 0005 1, 0012 1. Mostly crate names; `.vmf` in 0004, 0007, 0008, 0010. ADR 0011 also lists six crates not created yet | 131 | Accepted text is not edited. Handled by the name map in ADR 0013 and Status-line pointers on 0011 and 0004 |
| 16 | `docs/` other | `technical-debt.md` 21, `design-system.md` 3, `adr/index.md` 3 (the index tables name 0011's crates), `requirements.md` and `guides/` 0 | 27 | Low. Rewritten mechanically |
| 17 | `specs/` | 15 folders (0001–0007 plus eight slug folders), 31 files, 323 occurrences; no folder or file name contains the old name; 4 spec files hold PR links to `Th3Link/vectormanufactoring/pull/{3,7,10,25}` | 323 | Low. PR links stay (GitHub redirects). Four plans hold the working-directory path (see 22) |
| 18 | `.vmf` extension in code and tests | 136 lines in 21 `.rs` files (constants, dialog filter, fixture paths in 20 test files, doc comments); Rust identifiers: `NotAVmf` 16, `VMF_*` 8, about 20 test function and helper names (`not_a_vmf`, `build_vmf_zip`, `craft_vmf_with_raw_point_count`, `to_vmf_bytes`, ...); docs and specs 73 lines in 23 files | 225 occurrences, 50 files | Tests assert fixture file names: rename files and strings in the same commit |
| 19 | Golden fixtures | `curvyo-document-core/tests/fixtures/`: `valid`, `format_version_1`, `paths_v2`, `primitives_v3`, `rotation_v5`, `malformed_paths`, `future_format_version`, `truncated` as `.vmf`; `not_a_vmf.txt`. `generate_golden_fixtures` (ignored test) writes them by name | 9 files | **Bytes carry no old name** (manifests are `{"format_version":N,"loro_snapshot_version":1,"app_version":"0.1.0"}`; the Loro snapshot and `document.json` contain no `vecmanf` or `vmf`). Rename only; hashes must be unchanged. `truncated` and `not_a_vmf.txt` are not zips |
| 20 | In-file identifiers (saved project container) | None. No magic string, mimetype or application tag. `manifest.json` has three numeric/version fields; members are `manifest.json`, `document.loro`, `document.json` (keys: `format_version`, `size`, `objects`, per-shape fields; no product name). `unpack` rejects non-zips by signature (`PK\x03\x04`, `PK\x05\x06`), never by extension | 0 | The customer's own saved projects keep working: only the extension differs. No `format_version` bump, no migration. Future containers (keyring, sealed blobs) must not embed the old name |
| 21 | Persisted app data, keychain, config | None written today: no app directory, no config, no keychain entry, no `localStorage`. Planned (ADR 0004 §12 data directory, ADR 0007 keychain service) and not yet implemented | 0 | The identifier decision (D3) fixes the future directory names; do it before they exist |
| 22 | Absolute working-directory path `/home/marc/workbench/vecmanf-claude` | 22 lines in 9 tracked files: `CLAUDE.md`, `.claude/agents/implementer.md`, `.projectatlas/projectatlas.{claude.mcp,mcp,opencode}.json` (8 lines), 4 spec plans. Also keys, outside the repo, the Claude project directories and the auto-memory index (`MEMORY.md`) | 22 lines | Renaming the folder orphans session history and memory (keyed by path) and invalidates `.projectatlas` configs and every git worktree's stored path (`git worktree repair`). Recommendation D7: keep it |
| 23 | `.projectatlas/` | `projectatlas.db` (tracked binary index, holds file paths and so crate directory names); the three MCP JSON files carry the absolute path | 8 lines plus a binary | Regenerate after the rename; cannot be sed-ed |
| 24 | `.gitignore` | `vecmanf-app/gen/` | 1 | If missed, Tauri-generated `curvyo-app/gen/` becomes untracked noise |
| 25 | Git remote and repository | `origin https://github.com/Th3Link/vectormanufactoring.git`; the repo name already differs from the product name; no Renovate config, no badge, no Pages, no workflow referencing the repo slug | 1 remote | GitHub redirects web, git and API traffic after the customer's settings rename; update the remote URL. Do not create a new repo under the old name, it would end the redirect |
| 26 | Licence and copyright | `LICENSE` is the stock AGPL-3.0 text (no product name, no holder line); `license = "AGPL-3.0-or-later"` in `[workspace.package]` and `frontend/package.json`; no per-file headers (ADR 0006 §1) | 0 | Nothing to change |
| 27 | Domain and URLs | `curvyo.org` appears nowhere yet; no docs site, contact address or homepage URL in manifests or README | 0 | Only the bundle identifier uses it (D3). A site, mail or `homepage` fields are out of scope |
| 28 | Environment variables, cargo features | none carry the name: features are `default` and `custom-protocol` (Tauri scaffold); `WEBKIT_DISABLE_DMABUF_RENDERER` is a third-party variable; `CARGO_TERM_COLOR` | 0 | Nothing to change |
| 29 | npm and crates.io | nothing is published (`cargo publish` is denied in settings; `frontend` is private) | 0 | No registry name to claim or migrate. A crates.io/npm name check is only needed if publishing is ever decided |

## 2. Per-crate counts (lines containing `vecmanf`, case-insensitive, all tracked file types)

| Area | Files | Occurrences |
|---|---|---|
| `vecmanf-editor-wasm` | 46 | 315 |
| `vecmanf-ui-core` | 48 | 260 |
| `vecmanf-render-core` | 16 | 109 |
| `vecmanf-document-core` | 25 | 100 |
| `vecmanf-geometry-core` | 4 | 22 |
| `vecmanf-app` | 3 | 26 |
| `vecmanf-storage-io` | 4 | 6 |
| `frontend/` | 14 | 32 |
| `docs/` | 13 | 157 |
| `specs/` | 31 | 323 |
| `.github/`, `.claude/`, `.projectatlas/`, root files (`Cargo.*`, `deny.toml`, `CLAUDE.md`, `README.md`, `.gitignore`) | 11 | 64 |

## 3. Allow-list after the rename

`git grep -iE 'vecmanf|\bvmf\b|\.vmf'` must return only:

- ADR 0001–0012 bodies (accepted text), ADR 0013 and this appendix;
- `vecmanf-claude` path lines (22, category 22) while D7 keeps the directory;
- the four historical PR links to `Th3Link/vectormanufactoring` (they contain
  neither `vecmanf` nor `vmf`, listed here only so nobody "fixes" them).

## 4. Risks in one place

1. **Concurrency.** Any branch open during the window conflicts everywhere:
   hence one window, `main` frozen, no worktree but `base` and the rename's own.
2. **Silent CI shrink.** The `discover-core-crates` filter and required status
   check names depend on crate names; a miss gives green CI that checks less.
3. **Bundle identifier.** Cheap now, a migration after release (data, config,
   keychain, file associations). Decided in D3 on purpose.
4. **Old saved projects.** Content opens; only the extension changes. One
   `mv` loop for the customer; no converter, no alias by default.
5. **Stale generated output.** `frontend/src/wasm-bindings`, `frontend/dist`,
   `vecmanf-app/gen/`, `target/` keep old names until deleted or rebuilt.
6. **Working directory and memory.** Path-keyed; keep the directory (D7).
7. **Hand-reviewed strings.** 53 bare occurrences and the four open-error
   texts are user-visible and asserted by tests; they are reviewed, not
   scripted.
