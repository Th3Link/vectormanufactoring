# Rust multi-crate workspace starter guide

A reusable blueprint for structuring a Rust project with multiple related
components (e.g. "core logic" + "hardware/platform" pairs, or multiple
products sharing code) — crate separation, docs, ADRs, and CI/CD. Distilled
from restructuring a real project (a CAN-bus home-automation system: a
gateway and node firmware, unified into one workspace with shared crates).

Treat this as a checklist and a set of templates, not a rulebook — adapt
freely, but keep the *reasons* behind each choice, they're what make the
structure hold up as the project grows.

---

## 1. Crate separation

### The core/hardware split

If a component has both pure logic and platform-specific I/O (hardware,
network, filesystem, ...), split it into two crates from day one:

- **`<component>-core`** — `#![no_std]` (or otherwise platform-independent),
  zero I/O, deterministic (callers pass in `now`/state explicitly rather
  than the crate reading a clock or touching hardware). Fully testable
  with `cargo test` on a plain host toolchain. This is where correctness
  actually gets verified — cheaply, in CI, without hardware.
- **`<component>-hardware`** (or `-cli`, `-server`, whatever the platform
  is) — the thin, hard-to-unit-test layer that drives real I/O using the
  `-core` crate's types and pure functions. Kept as small and boring as
  possible; its job is wiring, not logic.

**Why this pays off**: the core crate gets real regression tests pinning
every quirk and edge case, runs in CI in milliseconds on stable Rust, and
can be read/reviewed without knowing anything about the target platform.
The hardware crate stays thin enough that bugs in it are usually obvious
from inspection.

### When you have *multiple* components (e.g. two products sharing a bus/protocol)

Once you have two or more `-core`/`-hardware` pairs, watch for code that's
actually identical (or should be) between them — most often a wire format,
protocol, or infrastructure boilerplate (config storage, logging, shared
data access). Extract that into:

- **`common-core`** — the platform-independent shared surface (wire
  format types, protocol codecs). Both `-core` crates depend on it and
  **re-export its modules at their historical paths**:
  ```rust
  // gateway-core/src/lib.rs
  pub use common_core::{can_id, can_message_type, device_type};
  ```
  This means every other file in the crate keeps using `crate::can_id::…`
  unchanged — the extraction is invisible outside `lib.rs`.
- **`common-hardware`** — shared platform boilerplate (e.g. exclusive
  access to a peripheral, a logging macro, a generic config-store
  singleton). Same re-export trick applies.

**Before extracting, actually diff the two implementations.** Independent
reimplementations drift — one side often has extra functionality the
other lacks (e.g. a hardware-filter helper only the node needs, or a
`from_name`/`name` helper only the CLI needs). Extracting the code means
merging as a **superset**, not picking one side and deleting the other's
extras. Keep genuinely different things (error types with different
shapes, command types that don't line up) separate — don't force a shared
shape onto code that only looks similar.

**Generic hardware plumbing pattern** — when two crates duplicate a
"singleton wrapping a resource, with typed accessors" pattern (e.g. a
config store), extract the *mechanism* as a const-generic/trait-bounded
type, and leave the crate-specific *shape* (the actual key enum, the
whole-struct load/default logic) where it is:

```rust
// common-hardware/src/config_store.rs
pub struct ConfigStore<const BUF: usize> { /* ... */ }
impl<const BUF: usize> ConfigStore<BUF> {
    pub async fn get_u8(&mut self, key: impl Into<u8>) -> Option<u8> { /* ... */ }
    // ...
}
pub struct ConfigCell<const BUF: usize>(Mutex<CriticalSectionRawMutex, Option<ConfigStore<BUF>>>);
```
```rust
// each-hardware-crate/src/config.rs
pub type Config = common_hardware::config_store::ConfigStore<128>;
static CONFIG: ConfigCell<128> = ConfigCell::new();
#[derive(IntoPrimitive)] #[repr(u8)] pub enum Key { /* crate-specific */ }
```

If a crate needs to add inherent-looking methods to a type it doesn't own
(the alias points at a foreign type), use a **local trait**, not an
inherent impl — Rust's orphan rules forbid the latter but allow the
former:
```rust
pub trait LoadOrInit { async fn load_or_init(&mut self) -> WholeConfig; }
impl LoadOrInit for Config { /* ... */ }
```
A `pub` trait using `async fn` will trigger `async_fn_in_trait` warnings;
if it's genuinely internal, mark it `pub(crate)` (note: a `[[bin]]` target
in the same package is a *separate crate* for visibility purposes — it
needs `pub`, not `pub(crate)`, to call it). If it must stay `pub`,
`#[allow(async_fn_in_trait)]` with a one-line justification is fine for a
single-threaded async runtime.

### Workspace layout

```
<repo>/
├── Cargo.toml                  # [workspace], workspace.package for edition/license/rust-version
├── <component-a>-core/
├── <component-a>-hardware/
├── <component-b>-core/
├── <component-b>-hardware/
├── common-core/
├── common-hardware/
└── docs/                       # see §3
```

One shared `Cargo.lock` for everything, even when different crates need
different toolchains (see §4) — CI just `cd`s into each crate's directory
per job, and a `rust-toolchain.toml` in a crate's own directory only
affects invocations from that directory (and cascades to its whole
dependency graph for that invocation), so mixing a stable-only crate and
an embedded-toolchain-only crate in one workspace works fine.

```toml
# root Cargo.toml
[workspace]
resolver = "2"
members = ["common-core", "common-hardware", "component-a-core", "..."]

[workspace.package]
edition = "2021"
license = "GPL-3.0-or-later"
rust-version = "1.97"
# Omit `version` here if components version independently — give each
# crate its own explicit `version = "..."` instead of `version.workspace = true`.

# Cargo only reads [profile.*] from the workspace root, never from member
# manifests — put release/LTO tuning here once.
[profile.release]
codegen-units = 1
lto = true
opt-level = 3
```

---

## 2. Architecture Decision Records (ADRs)

One global, numbered sequence — regardless of which crate/component an
ADR is actually about. A **Scope** column (or physical location) tells you
whether a decision is workspace-wide or component-specific; the numbering
itself stays flat so you never have to ask "which ADR-0003 do you mean".

**Template** (every ADR, no exceptions):

```markdown
# ADR NNNN: <short decision title>

**Status:** Accepted | Proposed | Superseded by [ADR MMMM](...)

## Context
What situation/constraint led here. Include the option(s) considered and
rejected, and *why* — future-you needs the rejected alternative's reasoning
as much as the chosen one's.

## Decision
What was actually decided, stated plainly.

## Consequences
What this costs, what it enables, what it doesn't solve. Be honest about
trade-offs accepted, not just benefits.
```

**Rules that keep this useful over time:**
- **Once accepted, an ADR's decision text is not edited.** If circumstances
  change, write a *new* ADR that supersedes it, and go back to mark the
  old one's Status line — don't rewrite history.
- **Placement**: workspace-wide ADRs live in the root `docs/adr/`;
  component-scoped ones live in `<component>-core/docs/adr/`. Each level
  gets its own `index.md` (a table); the root index aggregates *all* of
  them with a Scope column and links out to the component-scoped ones.
- **A merge/refactor is itself ADR-worthy.** "We extracted a shared crate,
  superseding the earlier 'keep independent' decision" is exactly the kind
  of thing to write down — it's easy to forget *why* two things were split
  apart or brought together six months later.

---

## 3. Requirements & other docs

Same split as ADRs: a short, workspace-wide `docs/requirements.md` (what
the *system* does, cross-cutting constraints, links out) plus a fuller,
component-scoped `requirements.md` per component. Don't let the root
document balloon — if a requirement is really about one component, it
belongs in that component's own doc with just a link from the root.

**Suggested root `docs/` tree:**
```
docs/
├── index.md              # landing page: what is this, links to every component
├── requirements.md        # system-level only
├── protocol-*.md          # anything genuinely shared (e.g. a wire format
│                           # used by *every* component, not just one side
│                           # of a conversation)
└── adr/
    ├── index.md
    └── NNNN-*.md
```

**Suggested per-component `docs/`:**
```
<component>-core/docs/
├── requirements.md         # this component's own detail
├── protocol-*.md           # anything one-sided (e.g. this component's
│                            # specific transport/API, not shared with others)
├── technical-debt.md        # known problems + the ADR that plans to fix each
└── adr/
    └── index.md
```

A **technical-debt.md** worth having: one section per known problem, each
ending with a link to the ADR (Status: Proposed) that plans to fix it.
This turns "things we know are bad" into a tracked, linked backlog instead
of tribal knowledge.

### Embedding docs into `cargo doc`

Keep documentation as plain `.md` files (readable directly on GitHub/any
editor) but splice them into each crate's rustdoc output too, so
`cargo doc` produces one browsable site instead of API docs plus a
separate, disconnected wiki:

```rust
// <crate>/src/docs.rs — every item is an empty marker module; no code.
//! Project documentation, pulled into `cargo doc` output via
//! `#[doc = include_str!(...)]`.

/// Workspace-wide requirements.
#[doc = include_str!("../../../docs/requirements.md")]
pub mod requirements {}

pub mod adr {
    #[doc = include_str!("../../../docs/adr/index.md")]
    pub mod index {}
    #[doc = include_str!("../docs/adr/0001-something.md")]
    pub mod adr_0001_something {}
}
```

Then `pub mod docs;` in `lib.rs`. Gotchas:
- `include_str!` paths are relative to the **source file**, not the crate
  root — count directory levels carefully (`<repo>/<component>/src/docs.rs`
  needs `../../../docs/...` to reach the repo-root `docs/`, but only
  `../docs/...` to reach its own component-level `docs/`).
- Build with `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` in CI — this
  catches both broken intra-doc links *and* bad `include_str!` paths as
  hard errors, immediately, instead of a silently-missing page.
- Rustdoc treats a bare ` ``` ` fence inside included markdown as a Rust
  doctest by default. Tag anything that isn't Rust: ` ```text `.
- Every markdown cross-link inside an included file must also account for
  the include site's directory depth, not the original file's — moving a
  doc file (e.g. during a reorg like this one) means re-checking every
  relative link inside it, not just the `include_str!` path pointing at it.

### Publishing to GitHub Pages

One job per crate builds `cargo doc --no-deps` (using whatever toolchain
that crate needs — see §4), uploads it as a build artifact; one
`build-site` job downloads all of them, writes a hand-rolled `index.html`
linking to each crate's `<crate_name>/index.html`, and
`actions/deploy-pages` publishes the combined result. See §4 for the full
workflow.

---

## 4. CI/CD pipelines

Structure jobs **by what toolchain a crate needs**, not by crate identity —
group `-core` crates (host-testable, plain stable Rust) separately from
`-hardware`/embedded crates (need a pinned custom toolchain, can't run
`cargo test`/`cargo msrv` at all since there's no native target). Use a
matrix within each group so adding a new crate of an existing kind is a
one-line change, not a new job.

**`ci.yml`** — the core loop:
```yaml
jobs:
  core-fmt-clippy:
    strategy:
      matrix:
        crate: [common-core, component-a-core, component-b-core]
    defaults:
      run:
        working-directory: ${{ matrix.crate }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings

  core-coverage:      # cargo-llvm-cov + nextest, uploads lcov.info
  core-msrv:          # cargo-msrv verify against the declared rust-version

  hardware-fmt-clippy:
    strategy:
      matrix:
        crate: [common-hardware, component-a-hardware, component-b-hardware]
    steps:
      - uses: <platform-specific-toolchain-action>
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings   # build, NOT --all-targets — a
                                            # no_std crate has no host test
                                            # target; --all-targets fails
                                            # trying to build one

  firmware-build:      # only for crates with a [[bin]] — matrix over
                        # {crate} x {debug, release}, archive artifacts
```

**`audit.yml`** — `rustsec/audit-check` (advisories) + `cargo-deny`
(license compliance) per crate, matrixed the same way. Each crate gets its
own `deny.toml` with an explicit license allow-list — a new dependency
pulling in an unreviewed license fails CI instead of silently shipping.

**`release.yml`** — triggered on a tag. If multiple components each
produce a release artifact from the *same* tag, matrix the build but
serialize the actual release-upload step (`max-parallel: 1`) — two
concurrent jobs both creating/updating the same GitHub Release for one tag
race each other.

**`docs.yml`** — see §3's "Publishing to GitHub Pages". Separate
`docs-core`/`docs-hardware` job groups (different toolchains), a
`build-site` job with `needs: [docs-core, docs-hardware]` that downloads
every crate's artifact and assembles the landing page, then
`configure-pages` → `upload-pages-artifact` → `deploy-pages`.

**`renovate.yml`** — self-hosted Renovate on a daily cron, reading the
repo-root `renovate.json` (`{"extends": ["config:recommended"]}` is a
perfectly good starting point).

---

## 5. Bringing multiple repos together (if you ever need to)

If components started as separate repositories and you're consolidating,
`git subtree add --prefix=<path> <remote> <branch>` (no `--squash`)
preserves full commit history — but understand what it actually does
before you rely on `git log --follow` to prove it:

- It does **not** rewrite each historical commit's file paths to include
  the new prefix. The prefix only exists in the merge commit's resulting
  tree. `git log --follow -- new/prefixed/path.rs` will only show the
  merge commit itself.
- The **original history is still fully there and reachable** — walk the
  merge commit's second parent directly (`git log <merge-commit>^2 --
  original/unprefixed/path.rs`) and every original commit, author, and
  message is intact. This is expected `git subtree` behavior, not data
  loss — just budget time to verify it this way instead of trusting
  `--follow` on the new path.
- Land each source repo as a plain sibling directory first (e.g.
  `_source-repo-name/`) and verify history before doing any restructuring
  (renames, extraction, deletions) as separate, ordinary commits on top.
  Keeps the history-preserving step isolated and easy to sanity-check
  before the harder-to-review file-shuffling begins.
- Before merging, check every source repo's **actual** state, not what you
  remember: uncommitted local changes, unpushed commits, multiple local
  checkouts that have diverged, which remote is actually canonical.
  Reference-only/vendored copies that were already excluded by convention
  (documented as "reference only, will be removed") often aren't part of
  the real git history at all — confirm before assuming they need
  handling.

---

## 6. General principles that held up

- **Extraction is a merge, not a move.** When unifying near-duplicate code
  across components, diff first. Assume drift exists until you've checked.
- **Preserve public paths across a refactor.** A `pub use` re-export at
  the crate root keeps every external call site unchanged even when the
  actual implementation moves to a shared crate — this is what makes a
  large extraction a safe, mechanical-feeling change instead of a
  find-and-replace across the whole codebase.
- **Verify with the real toolchain, not just `cargo check`.** An embedded/
  cross-compiled crate can type-check fine and still fail to *link*
  without the actual platform toolchain on `PATH` — run the full
  `build`/`clippy`/`fmt --check` loop with the real toolchain sourced
  before calling a crate done.
- **A decision that gets reversed is still worth documenting** — as a new
  ADR that explicitly supersedes the old one, not a silent code change.
  The reasoning for *why* something changed is exactly what the next
  person (or you, in a year) will need and won't have.
- **Don't force a shared shape onto genuinely different things.** Two
  crates each having an `error.rs`/`relais.rs`/`command.rs` doesn't mean
  they should share one — only extract what's actually the same contract,
  and say explicitly (in the ADR, in the module doc) what was deliberately
  kept separate and why.
