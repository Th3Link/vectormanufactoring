# Architecture Decision Records

One global, flat numbering sequence for the whole workspace, regardless of
which crate a decision is about (see
[`../guides/rust-workspace-blueprint.md`](../guides/rust-workspace-blueprint.md)
§2). Workspace-wide ADRs live here; a crate-scoped ADR lives in
`<crate>/docs/adr/` and is listed in this table with its scope and a link.

**Once an ADR is accepted, its decision text is not edited.** Changed
circumstances get a new ADR that supersedes the old one, and the old one's
Status line is updated to point at it. ADRs 0001–0011 below are accepted, so
this rule is now live: every further change to any of these decisions is a new
ADR. ADR 0012 was rejected before acceptance, so that rule never applied to it.
(0001–0010 were accepted by customer sign-off; 0011 is downstream assembly of
them and was accepted by the architect — see its own preamble for why
`CLAUDE.md` §3 does not require a customer round for it.)

**Name map (ADR 0013, 2026-10-07):** the product is called Curvyo. Read
`vecmanf-*` as `curvyo-*` (crates and directories) and `.vmf` as `.curvyo`
(file extension) in ADRs 0001–0012, whose text keeps the old names.

A decision too small for a full ADR gets a short dated note in that feature's
`specs/<NNNN-feature-slug>/adrs.md` instead.

| ADR | Title | Scope | Status |
|---|---|---|---|
| [0001](0001-ui-framework-and-canvas-rendering.md) | UI framework and canvas rendering | workspace | Accepted |
| [0002](0002-document-model-units-and-svg-round-trip.md) | Internal document model, units and SVG round-trip | workspace | Accepted |
| [0003](0003-geometry-kernel-booleans-offsetting-vcarving.md) | Geometry kernel, boolean operations, offsetting and V-carving | workspace | Accepted |
| [0004](0004-persistence-and-cross-machine-sync.md) | Persistence, collaboration and cross-machine sync | workspace | Accepted |
| [0005](0005-extension-and-plugin-model.md) | Extension and plugin model | workspace | Accepted |
| [0006](0006-license.md) | License | workspace | Accepted |
| [0007](0007-asset-library-connector-and-credential-storage.md) | External asset-library connector and credential storage | workspace | Accepted |
| [0008](0008-end-to-end-encryption-of-sync-and-collaboration.md) | End-to-end encryption of sync and collaboration | workspace | Accepted |
| [0009](0009-concurrent-editing-semantics.md) | Concurrent editing semantics — undo, ephemeral state and merge granularity | workspace | Accepted |
| [0010](0010-document-keyring-admins-and-revocation.md) | Document keyring — participants, admins and key revocation | workspace | Accepted |
| [0011](0011-workspace-and-crate-layout.md) | Workspace and crate layout | workspace | Accepted |
| [0012](0012-pages-in-the-document-model.md) | Pages in the document model | workspace | Rejected (2026-10-05, dropped from MVP) |
| [0013](0013-rename-to-curvyo.md) | Rename the product to Curvyo (amends 0011 crate names, 0004 §1 file extension; [inventory](0013-rename-to-curvyo-inventory.md)) | workspace | Accepted |

## Customer sign-off, 2026-10-02

The customer worked through every batched question across five rounds. The
decisions each round produced are recorded in the ADRs themselves — each
affected ADR states in its own preamble what the customer directed and what the
architect chose within that direction — so they are not duplicated here. In
summary, the five rounds settled: Tauri over Dioxus (0001); CRDT plus relay with
Loro, and cloud sync as a required path (0004); plugins as day-one scope with an
unstable-then-frozen interface (0005); AGPL-3.0-or-later with no plugin linking
exception (0006); the asset connector as read **and** write, with OS-keychain
credential storage, a git forge as the first push target and the git wire
protocol rather than a forge API (0007); an end-to-end-encrypted, open-source,
self-hostable relay (0008); and participant revocation as a requirement,
implemented as a signed, git-like keyring log with multiple admins (0010).
ADR 0002 and the ADRs split out of it (0009) and out of 0008 (0010) were
reconciled internally rather than by customer direction.

The last two open sub-questions were put with a recommendation and a stated
default, and both defaults were accepted: a persistent per-room device public
key visible to the relay operator is acceptable (no accounts, email or
registration implied), and a participant is a **device** with a user-chosen
label rather than a person. Both are recorded in
[ADR 0010](0010-document-keyring-admins-and-revocation.md) under its answered
questions.

Two decisions were accepted on their stated defaults rather than by an explicit
instruction, and are called out here so nobody mistakes silence for
unawareness. Reversing either needs a new ADR, not an edit:

- **Peer-scoped Ctrl+Z** ([ADR 0009](0009-concurrent-editing-semantics.md) §1).
  Undo reverts the local peer's own commits only, as every CRDT editor does;
  reverting an arbitrary past change is an explicit history-view action in a
  later story (0009's option C). With nobody else connected this is
  indistinguishable from classic linear undo, which is the customer's normal
  case.
- **E2EE in the first collaboration story** rather than after it
  ([ADR 0008](0008-end-to-end-encryption-of-sync-and-collaboration.md), open
  question). Deferring duplicates the join-path work rather than saving it.

## Open follow-ups

- **The workspace itself is now written but not created.**
  [ADR 0011](0011-workspace-and-crate-layout.md) fixes the twelve-crate layout,
  the full dependency direction, the wasm32 checks and the frontend's place in
  the build; the root `Cargo.toml` and the crate directories it describes are
  the first commit of product work, not part of the ADR. Three crate slots are
  deliberately left empty there (a machine/toolpath core crate and a device
  `-io` crate, awaiting the first machine-family ADR; `curvyo-model-core`;
  a plugin-host core crate), each with its trigger named in 0011 §8.
  One crate name the index's earlier list had missed is in it:
  `curvyo-vectorize-core`, named by ADR 0003 §6.
- ~~`CLAUDE.md` §8's crate-suffix list needs a lead amendment~~ — done: §8 now
  names `-wasm` (0001 §3), `-server` (0004 §5) and the suffix-less plugin SDK
  (0005 §1) alongside `-core`/`-app`/`-io`, pointing at ADR 0011.
- **Dangling cross-references in ADR 0004** (found while writing
  `specs/0001-project-file-foundation/adrs.md`, 2026-10-02): lines citing "ADR 0002
  §12" and "§12–§16" should point at ADR 0009, where peer-scoped undo and
  concurrent-editing semantics actually live — 0002's decision list ends at
  §11. Accepted decision text isn't edited, so this is a corrected pointer for
  readers, not a substantive change; fix opportunistically whenever 0004 is
  next touched for an unrelated reason.
- ~~**Boolean-crate spike (`spike/booleans`).**~~ — done, 2026-10-04: all three
  candidates (`i_overlay`, `geo`'s boolean ops, `clipper2-rust`) handled every
  degenerate fixture with no panic, hang or divergent output, and all three
  build cleanly for `wasm32-unknown-unknown`. The result is **`clipper2-rust`**,
  tie-broken on `#![forbid(unsafe_code)]` (vs. internal `unsafe` in the other
  two) and on being the Clipper2 algorithms directly rather than `geo`'s wrapper
  over an older, pinned `i_overlay`. Recorded as the dated note ADR 0003 §3 asks
  for, written there (not in a `specs/<NNNN-feature-slug>/adrs.md`, since slice 6
  doesn't exist as a story yet). This reopens the §4 offsetting question below —
  still pending the customer, not resolved by this spike.
- **Offsetting may reopen ADR 0003 §4 (customer decision, not yet asked).** Now
  that the boolean spike has picked `clipper2-rust`, its ported Clipper2
  offsetter covers the join and cleanup cases §4 decides to build on `kurbo`
  stroke expansion. Using it would change accepted text, so it needs an ADR
  superseding 0003 —
  noted in 0003 §4, pending the customer now that the spike result is in.
- **ADR 0007 is over the five-minute rule and wants splitting, not trimming.**
  It carries three subjects — credential storage, the source/sink trait design,
  and the git-forge sink — and a consolidation pass took out the prose without
  getting it under the limit. The clean fix is a new ADR for the git-forge sink
  superseding 0007 §§14–16; it is a readability follow-up, not a correction, so
  it waits until a story touches that area.
- **Plugin interface freeze (`1.0`).** ADR 0005 §3 ships the interface as
  `0.x` and names the freeze trigger; the freeze itself is a follow-up ADR.
- **Plugin capability grants.** Network and filesystem access for plugins are
  denied by ADR 0005 §7. Lifting that needs its own ADR plus a consent surface,
  triggered by a story.
- **Reverting a collaborator's change.** ADR 0009 §1's option C — an explicit
  "revert this change" action in a read-only history view, able to target any
  past commit — is a story, not shipped behaviour, and is never bound to Ctrl+Z.
- **Forward secrecy.** The one gap ADR 0008 names that ADR 0010 does not close:
  participants keep every epoch key they held, deliberately, so a disclosed key
  still opens that epoch's stored log. MLS (ADR 0008 option C) remains the
  destination if group management ever becomes a real product area, and nothing
  in ADR 0010 points away from it. A follow-up ADR, triggered by a story.
- **Keyring fork resolution needs a review before it ships.** ADR 0010 §5's
  three merge rules are where a bug re-admits a removed participant. They are
  pure functions over an entry set, so the review and exhaustive small-case
  tests are the mitigation, and they share the security-review slot with
  `curvyo-crypto-core`.
- **Push flow details for the git-forge sink.** The target is settled (ADR 0007
  §14, git wire protocol). Two sub-questions remain with working defaults:
  commit-and-push versus a real pull-request flow (default: commit and push, a
  PR needs a forge-specific API and its own story), and HTTPS token versus SSH
  key (default: token, which the credential design already covers).
- **Fonts have no identified asset provider** (ADR 0007 open question 3). A
  product question, not an architecture one: a font provider, if one appears, is
  one more `AssetSource` and needs no new decision. Until then the font
  collection is the local directory of ADR 0004 §12.
- **Is the customer going to *be* an asset provider?** (ADR 0007 open question
  2.) Default assumption: no — we build the client only. Becoming a provider
  means accounts and payments, which is a materially larger scope and its own
  ADR.
- **Machine families.** Each family (laser, cutting plotter, embroidery, CNC)
  gets its own ADR when its first story is ready. No shared machine
  abstraction is designed before the second family exists (`CLAUDE.md` §6).
- **`CLAUDE.md` §7 and §8 need amending by the lead.** ADR 0001 adds a
  TypeScript frontend and an npm dependency tree; the quality gate and the
  repository-layout section currently describe a Rust-only workspace.
