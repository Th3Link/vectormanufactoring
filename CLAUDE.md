# CLAUDE.md

Vector design and manufacturing application: one tool for drawing, preparing
and producing vector work on laser cutters, cutting plotters, embroidery
machines and CNC mills. Desktop first (Linux, Windows, macOS), browser second,
Android/iOS companion later.

This file is loaded into every session. Keep it short. Detail belongs in
`docs/`; link to it from here.

---

## 1. Language

- **Everything in the repo is English**: code, comments, docs, commit
  messages, issues, PR descriptions, UI strings.
- **Talk to the customer in German.**
- Plain, specific wording in docs and UI copy. No marketing language, no
  filler. Run `/no-slop` on README, docs and user-facing text before a PR.

## 2. Roles

| Who | Does | Does not |
|---|---|---|
| **Customer** (human) | Sets goals, decides scope, accepts stories, approves ADRs marked `needs-customer` | Write code or tickets |
| **Lead** (main session) | Plans, delegates to subagents, integrates, reports to customer | Write product code beyond trivial glue |
| `product-owner` | Vision, backlog, stories with acceptance criteria, proposes features that make the product great | Touch `src/` |
| `architect` | ADRs, crate layout, reviews structural changes, guards the rules in §5 | Implement features |
| `implementer` | Implements one story at a time, test-first, in its own worktree | Change scope or acceptance criteria |
| `ux-engineer` | Design system, interaction design, UI review of every story with UI | Change domain logic |
| `tester` | Tests against acceptance criteria, edge cases, regressions; reports PASS/FAIL | Fix production code |

Agent definitions live in `.claude/agents/`. Subagents return a summary of at
most ~30 lines plus file paths. Full reasoning goes into files, not into the
lead's context.

## 3. Autonomy: decide or ask

The customer wants to do as little coordination as possible. Default to
deciding.

**Decide without asking:**
- Anything inside the scope of an accepted story and reversible in one PR.
- Naming, module structure within a crate, test design, refactorings that
  keep behaviour.
- Adding a dependency with a permissive license that passes `cargo deny`
  (justify it in the PR).

**Ask the customer:**
- New features or scope changes (the PO proposes; the customer decides).
- ADRs on platform, UI framework, document model, persistence/sync,
  plugin model, license, accounts/payment.
- Anything involving user accounts, paid content, telemetry, or data leaving
  the machine.
- Removing or significantly changing user-facing behaviour that was accepted.

**How to ask:** batch questions into one message. For each question give
options, your recommendation, and what happens if the customer does not
answer (the default you will take). Never block on a question that has a
safe default.

## 4. Workflow

**Story lifecycle** (one file per story in `docs/backlog/NNNN-slug.md`):

1. **Ready** — PO wrote user value, acceptance criteria (testable, numbered),
   and out-of-scope notes. UX notes attached if the story has UI.
2. **Build** — implementer works on branch `story/NNNN-slug` in a git
   worktree. Tests first for core logic.
3. **Verify** — tester writes acceptance tests from the criteria without
   reading the implementation first, then runs the full gate (§7).
   `ux-engineer` reviews UI. `architect` reviews if crates, public APIs,
   dependencies or the document model changed.
4. **Demo** — lead posts to the customer: what changed, how to try it
   (command or screenshot), open issues. One short message.
5. **Done** — customer accepts; PR is squash-merged; story file gets
   `Status: Done` and the PR link.

**Definition of Done:** all acceptance criteria covered by tests, gate green,
docs updated where behaviour changed, no new `TODO` without an issue link,
customer accepted.

Parallel work: at most two implementers at once, each in its own worktree,
on stories that do not touch the same crates.

## 5. Engineering rules

SOLID, KISS and YAGNI in concrete, checkable form:

- **No trait without a second implementation** (or a test double that is
  actually used), unless an ADR requires the abstraction.
- **No generic parameter** unless at least two concrete types use it now.
- **No new crate without an ADR.** No speculative feature flags or config
  options.
- **One responsibility per module.** The module doc comment states it in one
  sentence. If the sentence needs "and", split the module.
- Functions over ~60 lines or modules over ~500 lines need a reason in the
  PR or get split.
- **Dependency direction is one-way:** `*-core` crates never depend on UI,
  platform or I/O crates. The crate graph enforces this.
- **Errors:** `thiserror` in libraries, `anyhow` only in binaries. No
  `unwrap`/`expect` outside tests unless preceded by an
  `// invariant: ...` comment.
- **Units are types.** Lengths, angles, speeds and powers are newtypes, never
  bare `f64`. Every geometric comparison uses an explicit tolerance.
- Core crates use `#![forbid(unsafe_code)]`. `unsafe` anywhere else needs an
  ADR.
- Delete dead code. No commented-out code.
- Every file-format importer/exporter (SVG, DXF, G-code, HPGL, embroidery
  formats, ...) gets golden-file tests in `tests/fixtures/`.

Workspace lints (root `Cargo.toml`) enforce what can be enforced:

```toml
[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "warn"
```

## 6. Architecture constraints

These hold until an ADR says otherwise.

- **Core/platform split** as described in
  `docs/guides/rust-workspace-blueprint.md` §1. Domain logic (document
  model, geometry, vectorization, toolpaths, machine output formats,
  material database logic, font classification) lives in `*-core` crates:
  no filesystem, network, clock, threads or UI. Callers pass everything in.
- **Every `*-core` crate builds for `wasm32-unknown-unknown`** as well as the
  host. This keeps the browser target possible and is checked in CI.
- **Targets by priority:** desktop (Linux, Windows, macOS) → browser →
  mobile companion. Mobile gets no code until a story needs it.
- **Machine support is layered:** job/toolpath generation in core; output
  formats (G-code dialects, HPGL, embroidery formats) in core; device
  communication (serial, USB, network) in platform crates. The browser may be
  export-only where no device API exists.
- **Implement one machine family at a time**, driven by stories. No
  "universal machine abstraction" before the second family exists.

Initial ADRs the architect proposes (all `needs-customer`), before any
product code:

1. UI framework and canvas rendering (must serve desktop and browser; state
   how `ui-ux-pro-max` guidance applies to the choice)
2. Internal document model, units and SVG round-trip strategy
3. Geometry kernel and boolean operations; path to offsetting and
   Voronoi-based V-carving
4. Persistence and cross-machine sync (projects, material database, asset
   libraries)
5. Extension/plugin model
6. License

## 7. Quality gate

Run before reporting a story as done. Quiet flags keep output (and tokens)
small; show only failures to the lead.

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --message-format=short -- -D warnings
cargo nextest run --workspace --status-level fail
cargo build -q --target wasm32-unknown-unknown -p <each-core-crate>
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc -q --workspace --no-deps
```

Hooks in `.claude/settings.json` format every edited `.rs` file and run
clippy and tests when the lead or a subagent finishes. A failing hook
means: fix it, do not work around it.

## 8. Repository layout and docs

Follow `docs/guides/rust-workspace-blueprint.md` for workspace layout, ADR
format and numbering, requirements split, embedding docs into `cargo doc`,
and CI structure. Adaptations for this project:

- Crate suffixes are `-core` (pure logic), `-app` (UI/binaries) and `-io`
  (device and OS integration). There is no `-hardware` suffix and no
  embedded toolchain.
- CI runs the app crates on an OS matrix (ubuntu, windows, macos) and adds
  the wasm32 build of core crates.
- `docs/backlog/` holds stories; `docs/design-system.md` holds the UI design
  system.

Use the ProjectAtlas MCP for code navigation before broad grep/read passes.

## 9. Git and GitHub

- Trunk-based. `main` is always releasable. Never push to `main` directly,
  never force-push shared branches.
- Branches: `story/NNNN-slug`, `fix/slug`, `chore/slug`.
- Conventional Commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`,
  `chore:`), scope = crate name where it applies.
- One PR per story. PR description: story link, what changed, ADRs touched,
  new dependencies with license and reason, how to try it.
- Squash-merge after green CI and customer acceptance. The customer may grant
  standing merge permission for `chore/` and `fix/` PRs; until then, ask.
- Renovate updates dependencies; the lead batches them into one weekly PR.

## 10. First session

Do not write product code yet.

1. `product-owner`: write `README.md` (story and motivation, what the
   project is and is not, status) and `docs/requirements.md` from the
   customer brief below. Split into system-level and component-level
   requirements per the blueprint. Mark every requirement Must/Should/Could
   and propose an MVP cut.
2. `architect`: propose ADRs 1–6 from §6 as `Proposed`.
3. Lead: send the customer one message with the MVP proposal and the
   batched ADR questions (§3).

After the customer's answers are in `docs/`, delete §11 from this file.

## 11. Customer brief (source material, remove after §10)

**Background.** The customer uses Inkscape to create and edit vector
graphics, Ink/Stitch to prepare them for an embroidery machine, exports to a
cutting plotter for cutting and drawing, and LightBurn for the laser cutter.
Pain points: Inkscape's UI/UX, especially extensibility; LightBurn is, per
the customer, no longer developed for Linux, and lacks cross-machine work
and material management. Material test patterns are needed once per machine
and material and should be stored and reused.

**Requirements as stated:**

- **Font management.** Work with many fonts without installing them all
  system-wide. Categorize fonts (e.g. fonts suitable for cutting, where
  glyphs are connected; initials; symbols and ornaments). Hide irrelevant
  system fonts while working on a project.
- **Vectorization and vector optimization** clearly better than Inkscape:
  combine several trace settings, and smarter path simplification.
- **Manufacturing as a core part:** laser cutting, laser engraving, cutting
  plotter, embroidery machine, CNC mill (2.5D, V-carving of text via Voronoi,
  e.g. OpenVoronoi).
- **Material management** per machine and material, including stored
  material test results. Works across the customer's computers.
- **Asset manager** for templates and finished files that get reused and
  composed. Additional libraries can be added, including subscription or paid
  ones with login.
- **Vector path and node editing.** Start small: Bézier paths, rectangles,
  circles, outlines, stroke width, stroke styles, colors, gradients, object to
  path, union, difference, intersection.
- Feature set comparable to LightBurn over time.
