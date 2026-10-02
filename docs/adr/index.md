# Architecture Decision Records

One global, flat numbering sequence for the whole workspace, regardless of
which crate a decision is about (see
[`../guides/rust-workspace-blueprint.md`](../guides/rust-workspace-blueprint.md)
§2). Workspace-wide ADRs live here; a crate-scoped ADR lives in
`<crate>/docs/adr/` and is listed in this table with its scope and a link.

**Once an ADR is accepted, its decision text is not edited.** Changed
circumstances get a new ADR that supersedes the old one, and the old one's
Status line is updated to point at it. All ten ADRs below are accepted, so this
rule is now live: every further change to any of these decisions is a new ADR.

A decision too small for a full ADR gets a short dated note in that feature's
`specs/<feature-slug>/adrs.md` instead.

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

- **ADR 0011 — workspace and crate layout.** The crates named by ADRs 0001–0010
  (`vecmanf-document-core`, `vecmanf-geometry-core`, `vecmanf-render-core`,
  `vecmanf-ui-core`, `vecmanf-library-core`, `vecmanf-crypto-core`,
  `vecmanf-storage-io`, `vecmanf-editor-wasm`, `vecmanf-sync-server`,
  `vecmanf-plugin`, `vecmanf-app`) plus the TypeScript frontend need one ADR
  that fixes the full dependency direction, the wasm32 CI matrix and the
  frontend's place in the build. It must also record ADR 0008 §5 as the single
  permitted exception to "the server depends on no core crate", and place
  ADR 0009 §2's awareness channel — which is I/O, so it cannot live in a
  `*-core` crate while the merge semantics it serves must. ADR 0010 §14 widens
  that exception's contents without widening the exception itself: the keyring's
  replay and merge rules are pure, so the server shares them. 0001–0010 are now
  accepted, so this is the next ADR to write. `CLAUDE.md` §5 forbids a new crate
  without an ADR, so no crate is created before then.
- **Boolean-crate spike (`spike/booleans`).** ADR 0003 §3 decides flattened
  polygons and a pure-Rust crate; the spike picks between `i_overlay` and
  `geo`'s boolean ops on degenerate input and wasm build cleanliness, before the
  first geometry story. The result is a dated feature-local decision in that
  story's `specs/<feature-slug>/adrs.md`, not an edit to the accepted ADR. A
  result rejecting both candidates would need an ADR superseding 0003.
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
  `vecmanf-crypto-core`.
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
