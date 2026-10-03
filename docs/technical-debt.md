# Technical debt

Known problems and deliberately accepted trade-offs, each with the ADR that
caused it and the ADR or story that would resolve it. An entry here is a
tracked decision, not a complaint — adding one is how an accepted shortcut
stays visible.

Nothing in this file is a defect in shipped code yet; the project has no
product code. These are the costs the accepted ADRs 0001–0010 choose to pay.

## Boolean results are polylines, not curves

Boolean operations flatten curves and return polygons
([ADR 0003](adr/0003-geometry-kernel-booleans-offsetting-vcarving.md) §3).
Manufacturing output is unaffected — everything is flattened before it reaches
a machine — but the *editable* result of a union has far more nodes than
Inkscape's and is not cleanly re-editable.

**Resolution:** a curve-refitting pass after boolean operations, as its own
story using `kurbo`'s curve fitting. Needs a quality bar defined first
(maximum deviation, node-count target).

## V-carve depth comes from an approximate medial axis

V-carving uses a constrained-Delaunay approximation of the medial axis rather
than an exact Voronoi diagram of curve segments
([ADR 0003](adr/0003-geometry-kernel-booleans-offsetting-vcarving.md) §5).
Quality depends on flattening density and will differ visibly from
OpenVoronoi on sharp interior corners.

**Resolution:** adaptive flattening near concave corners, judged against
reference cuts. Only worth doing if real cuts show the error; measure before
fixing. If it is not enough, the escalation is an exact segment-site Voronoi via
`boostvoronoi` (Boost.Polygon ported to pure Rust) with `centerline`'s
medial-axis filtering on top — re-checked 2026-10-02, still the only pure-Rust
route to what OpenVoronoi does, and still not free: `centerline` depends on
`rayon` and so does not build for `wasm32-unknown-unknown`, which would make
V-carve a desktop-only capability or force a reimplementation of that filter.
That trade is what makes it an escalation rather than the first choice.

## SVG round-trip is lossy

We own a documented SVG subset; unmodelled attributes survive in a per-node
passthrough bag and unparseable content is reported as a named loss
([ADR 0002](adr/0002-document-model-units-and-svg-round-trip.md) §10).
CSS-heavy and text-heavy files from other tools will not come back out
unchanged.

**Resolution:** driven by real files. Each failing customer file becomes a
golden-file fixture and either widens the modelled subset or is accepted as a
documented limit.

## Per-node resolved styles, no shared styles

Styles are resolved and stored per node, with no cascade and no named styles
([ADR 0002](adr/0002-document-model-units-and-svg-round-trip.md) §5).
"Change every red stroke at once" and reusable style definitions are a real
refactor of the style model, not a feature flag. Under replication there is a
second cost: a bulk restyle is one CRDT operation per affected node, so it adds
log growth proportional to the selection.

**Resolution:** a named-style story, if the customer asks for it. The cascade
stays rejected — it would also put a shared parent register in the path of
every concurrent style edit.

## Undo cannot reach a collaborator's change

Undo and redo are scoped to the local peer
([ADR 0009](adr/0009-concurrent-editing-semantics.md) §1): Ctrl+Z
reverts my last commit, never my collaborator's, and the stack is session-scoped
so it does not survive reopening the document. This is what every CRDT editor
does and it is the only semantics that cannot lose concurrent work, but it is a
real behavioural difference from a single-user editor, and "undo the last change
to this document" is not a thing we have.

**Resolution:** two stories, neither urgent. A history view over the persisted
operation log, so "what changed and by whom" is answerable after a reopen; and
an explicit "revert this change" action in that view that can target any past
commit, including another peer's — deliberately not bound to Ctrl+Z. Peer-scoped
Ctrl+Z itself is accepted, on ADR 0009's stated default (ADR index, customer
sign-off), so changing it would need a new ADR rather than a story.

## A contested scalar loses one side's value

Merge granularity is per field
([ADR 0009](adr/0009-concurrent-editing-semantics.md) §3). Path
anchors are a movable list and text is a text container, so concurrent work on
different nodes of a path or different characters of a string both survive. But
scalars — a transform component, stroke width, a colour, a job parameter — are
last-writer-wins registers. Two peers dragging the *same* node at the same
moment produce one of the two positions; there is no numeric merge. Nothing is
corrupted and nothing is unrecoverable (the log holds both), but one edit
visibly does not take effect, and "a CRDT does not lose work" is false at this
granularity.

**Resolution:** not a code fix — finer granularity than a field buys nothing
here, and averaging two drags would be worse than picking one. It is a UX
obligation: presence must make it visible that someone else is manipulating the
object you are about to grab (ADR 0009 §2's awareness channel already carries
what that needs). A design-system story owns it.

## A collaborator without the font sees substituted text

Fonts are referenced from a user-managed collection, not embedded by default
([ADR 0004](adr/0004-persistence-and-cross-machine-sync.md) §12), so a peer may
not hold a face the document uses. That peer renders a substitute and is blocked
from writing re-measured layout or converting the text to paths
([ADR 0009](adr/0009-concurrent-editing-semantics.md) §5) —
otherwise one peer's missing font silently rewrites the document for the peer
who has it. The restriction is correct and still means a collaborator can see
and edit a document they cannot render faithfully.

**Resolution:** an "embed fonts when sharing" action alongside the existing
export option, plus a visible substituted-font marker. Both are stories; the
write-back restriction must be enforced in `vecmanf-document-core`, not only in
the UI, because it is a correctness rule rather than a hint.

## Record conflicts are not merged automatically

Documents converge by CRDT, but the machine/material database and library
catalogs do not: they are per-record files moved as files
([ADR 0004](adr/0004-persistence-and-cross-machine-sync.md) §6, §7). Per-record
granularity and an append-only test-result log make conflicts rare, but editing
the same material offline on two machines produces a conflict the user must
resolve.

**Resolution:** detect and surface conflict copies in the UI with a
side-by-side resolution view (a story), on top of
`vecmanf-library-core`'s `merge()`. Extending the CRDT to records is the
expensive alternative and is not planned.

## Loro is a young dependency in the most expensive position

The document model is a Loro CRDT replica
([ADR 0004](adr/0004-persistence-and-cross-machine-sync.md) §2). Loro was
chosen over the more mature Automerge for its movable-tree support, which a
vector document's reparent and z-order operations need. Its snapshot format and
API are younger, and the format is one we must then support forever. The
reconciliation of ADR 0002 adds a second load-bearing dependency: node identity,
z-order and **undo remapping** are Loro's behaviour, not ours
([ADR 0002](adr/0002-document-model-units-and-svg-round-trip.md) §5 and
[ADR 0009](adr/0009-concurrent-editing-semantics.md) §1), so a
correctness bug in its undo manager is a correctness bug in our editor.

**Resolution:** containment is structural, not optimistic — no Loro type in any
public API (§3), and a Loro format change is treated as a format migration of
ours with golden-file tests (§9). The residual risk is an unplanned migration.
Re-evaluate before `1.0` of the application.

## We operate a server now

The collaboration relay and cloud sync
([ADR 0004](adr/0004-persistence-and-cross-machine-sync.md) §5) bring permanent
operational work that no code change removes: uptime, TLS renewal, backups
*with tested restores* of customer data, storage growth, abuse and rate
limiting, security updates on a network-facing service, and data-controller
duties (privacy policy, deletion on request, breach process).

End-to-end encryption
([ADR 0008](adr/0008-end-to-end-encryption-of-sync-and-collaboration.md))
narrows the duty but does not remove it. Whoever runs an instance is a data
controller for **traffic metadata and account-free session data** — room and
blob ids and sizes, IP addresses, connection and update timing, participant
counts, and therefore who worked with whom and when — and **not** for plaintext
content, which no instance can decrypt. [ADR 0010](adr/0010-document-keyring-admins-and-revocation.md)
adds per-room device public keys, fingerprints and the membership history to
that list, because the keyring log is public data the relay stores and
validates: a persistent pseudonymous identity per device per room, with no
email, password or registration behind it. Backups now hold ciphertext we cannot
read: they protect against our failures, not a user's lost key.

**Resolution:** none — this is a cost, not a defect. It is listed here so it
stays visible when someone estimates the project's ongoing burden. What keeps it
as small as it honestly can be: ADR 0004 §10 holds personal data to a room
token, a display name and a per-room device public key — no account, email or
password anywhere — and ADR 0004 §13 makes self-hosting a real alternative for
anyone who does not accept our instance holding even that.

## Revocation is forward-only: it cannot un-ring a bell

Removing a collaborator rotates the document key and re-wraps it for everyone
who remains ([ADR 0008](adr/0008-end-to-end-encryption-of-sync-and-collaboration.md)
§8, [ADR 0010](adr/0010-document-keyring-admins-and-revocation.md) §8), so the
removed device cannot read anything written afterwards. It can still read
everything it already had: the plaintext `.vmf` on its own disk, its cached
updates, and every epoch key it legitimately held. There is no mechanism — in
this product or in any encryption scheme — that retracts information somebody has
already decrypted.

This is listed as debt only because it is a gap between what "remove
collaborator" sounds like and what it does. The customer accepted it explicitly
("they get to keep the last state they had").

**Resolution:** none available, and none sought. What is actionable is wording:
the removal confirmation must state in one sentence that the person keeps what
they already have, and no UI string may say "access revoked" without that
qualifier. A UX story owns it.

## No forward secrecy, and rotation does not give it

Participants retain every epoch key they have held, deliberately, because the
relay's stored log and older snapshots are sealed under those epochs and must
stay readable to the people who were there
([ADR 0010](adr/0010-document-keyring-admins-and-revocation.md) §9). So a key
disclosed later still decrypts everything captured under that epoch, and the
relay's log is exactly such a capture. This remains the largest gap against the
"state of the art" the customer asked for.

**Resolution:** MLS (ADR 0008 option C) is the destination if group management
ever becomes a real product area; epochs, per-participant wraps and signed
membership are the same concepts it formalizes, so the move is conceptual rather
than inventive. A follow-up ADR, triggered by a story — not by this entry.

## Losing every admin key freezes a document's membership

An admin is the only role that can add, remove or rotate
([ADR 0010](adr/0010-document-keyring-admins-and-revocation.md) §12), and there
is deliberately no network path to re-grant admin without an admin key —
anything that could do so could also be done by a relay operator. If every admin
of a document loses both keychain and recovery key, the document keeps working
for its existing members but its membership can never change again. If every
*member* also loses their keys, the only copy left is an ordinary plaintext
`.vmf` on somebody's disk, recovered by copying the file by hand and forking it
into a new document with a new genesis entry (§13).

**Resolution:** not a code fix — it is the designed worst case. The mitigation is
social and belongs in the UI: a second admin is a backup, so the share flow must
push for one at the moment a document is first shared, and the recovery-key
export must be as hard to skip as ADR 0008 §9 requires. A UX story owns both.

## The keyring's fork resolution is where a bug becomes a security bug

Concurrent admin actions fork the keyring log, and the merge is settled by three
rules — deny wins, the admin set never empties, highest epoch wins with a hash
tiebreak ([ADR 0010](adr/0010-document-keyring-admins-and-revocation.md) §5).
A wrong tiebreak re-admits a removed participant or, in the other direction,
strands a document with no admin. This is authorization logic, which is the kind
of code whose failures tests do not stumble over by accident.

**Resolution:** the rules are pure functions over an entry set with no clock and
no I/O, so small membership graphs can be enumerated exhaustively in tests —
that, plus the security review slot shared with `vecmanf-crypto-core` below,
before the first collaboration story ships.

## No key recovery for encrypted cloud sync

The per-user sync key lives in the OS keychain with a one-time exported
recovery key and no escrow
([ADR 0008](adr/0008-end-to-end-encryption-of-sync-and-collaboration.md) §9).
A user who loses both has blobs on the server that nobody, us included, can
decrypt. The design that stops us reading the data is the same design that
stops us helping. The same recovery key now also covers the device identity
keys of [ADR 0010](adr/0010-document-keyring-admins-and-revocation.md) §1, so
losing it costs standing as a participant and an admin as well as readable
blobs — see the admin-freeze entry above.

**Resolution:** none available without weakening the guarantee. What is
actionable is UX: the recovery-key export must be impossible to skip past
accidentally and the consequence must be stated at setup, not in a FAQ. A UX
story owns that wording, alongside ADR 0007's keychain-fallback message.

## Late joiners depend on a client-produced snapshot

A blind relay cannot compact a CRDT log, so snapshots are sealed and uploaded
by clients
([ADR 0008](adr/0008-end-to-end-encryption-of-sync-and-collaboration.md) §6).
A room nobody has opened since creation has no snapshot and a joiner replays
the entire update log; the log only shrinks when some client produces a newer
snapshot and tells the server it may prune, which the server cannot verify.
Join time and server storage therefore depend on client behaviour.

**Resolution:** a snapshot policy story — when a client seals one (on close, on
log-length threshold), measured against a real long-lived room. Document growth
is the related entry below.

## `vecmanf-crypto-core` is our code in the path of every byte

The AEAD envelope, the header format and the invite encoding are ours
([ADR 0008](adr/0008-end-to-end-encryption-of-sync-and-collaboration.md) §4),
and so are the key wrap, the signed keyring entry encoding, the validity replay
and the merge rules
([ADR 0010](adr/0010-document-keyring-admins-and-revocation.md) §14). The
primitives are not — XChaCha20-Poly1305, X25519 and ed25519 from RustCrypto —
but the framing, the associated-data binding, the key handling and now the
authorization logic are, and framing mistakes are the usual way this class of
code fails. The crate is also the one `*-core` crate the server depends on
(ADR 0008 §5), so a bug there is a bug on both sides of the wire.

**Resolution:** a focused security review before the first collaboration story
ships, with fixed test vectors in the crate and a negative test per failure
mode: wrong document id, wrong epoch, replayed nonce, truncated tag, a wrap
transplanted to another recipient or epoch, an entry signed by a non-admin, an
entry signed by an admin who had already been demoted in its own ancestry, and
a replayed invite claim. Same review slot as the credential-storage fallback
below and the keyring merge rules above; they share the keychain module.

## Upload capability cannot always be known before trying

Whether a configured credential may push to a provider is checked at connect
time only where the provider exposes token scopes
([ADR 0007](adr/0007-asset-library-connector-and-credential-storage.md),
capability options). Elsewhere the user finds out from a rejected upload, after
committing to the action.

**Resolution:** none in general — it is the providers' API surface, not ours.
Per provider, prefer a scope check; where none exists, the error message is the
mitigation and is a UX concern. For the git sink there is no scope endpoint at
all without a forge API, so push capability is discovered from a rejected push:
case C by construction.

## The git sink depends on `gitoxide`'s youngest half

The remote `AssetSink` pushes over the git wire protocol
([ADR 0007](adr/0007-asset-library-connector-and-credential-storage.md) §14),
chosen so one implementation serves GitHub, GitLab, Gitea, Forgejo and bare
self-hosted remotes. Push is the least mature part of `gitoxide`, and the
fallback — `git2`/libgit2 — is a C dependency with no wasm build and would need
its own ADR. Two smaller costs come with it: the per-connection clone turns the
cache of §8 into a working copy that can be left conflicted by a rejected push
and that the user may also open in their own git tooling, and §10's no-force-push
rule means some conflicts end in "resolve this yourself" rather than in the
application.

**Resolution:** measure before the story, not after — push a real file to a real
Gitea remote and a real GitHub remote over HTTPS with a token, from Linux,
Windows and macOS. That result decides `gitoxide` or an ADR for `git2`. Golden
tests run against a local bare repository, which needs no network and covers
everything except the wire itself.

## Document files grow with edit history

A CRDT document carries operation metadata and tombstones, so a long-lived
`.vmf` grows beyond its content and costs more memory when open
([ADR 0004](adr/0004-persistence-and-cross-machine-sync.md) consequences). Undo
adds to this rather than shrinking it: an undo is a new forward commit, not a
rewind ([ADR 0009](adr/0009-concurrent-editing-semantics.md) §1), so
a long edit-and-undo session grows the log twice over.

**Resolution:** a compaction story using Loro shallow snapshots, with a
measured threshold. Measure on a real long-edited document first.

## Credential storage has a hand-rolled fallback

Third-party asset-library credentials go in the OS keychain, but where no
Secret Service exists (common on Linux, the primary platform) the fallback is
our own Argon2id + XChaCha20-Poly1305 file
([ADR 0007](adr/0007-asset-library-connector-and-credential-storage.md) §1).
That is our own crypto-using code holding somebody else's paid-service secrets.

ADR 0008 §7 and §9 put the epoch keys, the per-user sync key and ADR 0010's
device identity keys in the same store, so the fallback now also decides whether
a user's documents are readable at all and whether they can still act as an
admin — a bug there is data loss and loss of document control, not only a
credential leak.

**Resolution:** a focused security review of that module before it ships, plus
the redaction rules of §4 checked in review rather than assumed. Drop the
fallback entirely if a portable alternative appears.

## No local index for large libraries

Per-record files mean a large asset or font collection is thousands of small
files, listed and searched by walking the directory
([ADR 0004](adr/0004-persistence-and-cross-machine-sync.md) consequences).

**Resolution:** a rebuildable local index (SQLite in `vecmanf-storage-io`,
never authoritative), once a collection is big enough to measure the problem.

## Canvas performance on Linux/WebKitGTK is unverified

The shell is Tauri, so on Linux — the customer's primary platform — the canvas
runs on WebKitGTK, the weakest of the three engines for sustained WebGL2 work
([ADR 0001](adr/0001-ui-framework-and-canvas-rendering.md), canvas section).
WebGPU is not reliably available there, so WebGL2 is the baseline. Headless
rendering for golden-image tests is also assumed rather than proven.

**Resolution:** measure with a real stress scene (thousands of nodes, live node
drag, zoom) before the first canvas story. This is the one result that would
invalidate ADR 0001 rather than cost a refactor, so it is measured first, not
discovered later.

## The quality gate covers only half the product

`CLAUDE.md` §7 describes a Rust-only gate, but ADR 0001 adds a TypeScript
frontend and an npm dependency tree. `cargo deny` cannot see npm licenses, and
no lint, type-check, test or license check currently runs on the frontend.

**Resolution:** the lead amends `CLAUDE.md` §7 and §8 to add `tsc --noEmit`,
ESLint, Prettier, `vitest`, Playwright and an npm license/audit check with
ADR 0006 §2's allow-list. Until then a green Rust gate is not a green build.

## Accessibility is partly ours to build

The engine supplies the DOM accessibility tree, keyboard behaviour, text input
and IME, and `ui-ux-pro-max`'s `react`/`shadcn`/`html-tailwind` data applies
directly — a large improvement over the rejected Rust-GUI options. What is
still ours: the canvas itself is one opaque element to a screen reader, and
selection, node editing and tool state have no accessible representation
([ADR 0001](adr/0001-ui-framework-and-canvas-rendering.md) §4).

**Resolution:** `docs/design-system.md` states the canvas accessibility
strategy (DOM overlays for handles and text, keyboard-only node editing)
before the first canvas story, and UI review checks it.

## Four transitive dependencies are unmaintained, with no safe upgrade

`cargo deny`'s first real run (`project-file-foundation`, the first slice to
pull in `loro` and `tauri`) surfaces four RustSec "unmaintained" advisories,
none with a fix we control: `im` (RUSTSEC-2026-0248), `sized-chunks`
(RUSTSEC-2026-0251) and `bitmaps` (RUSTSEC-2026-0247), all pulled in by
`loro-internal`'s persistent-map use; and `proc-macro-error`
(RUSTSEC-2024-0370), pulled in by `glib-macros` via Tauri's Linux WebKitGTK
stack (`gtk`/`glib`). None are security vulnerabilities — "unmaintained"
only — and none have an upstream successor `loro` or `gtk-rs` has adopted
yet.

**Resolution:** acknowledged explicitly in `deny.toml`'s `[advisories]`
`ignore` list, each with the RUSTSEC id and this rationale, rather than
silently passing or silently failing CI. Re-check at each `loro`/`tauri`
upgrade: `im` has a maintained fork (`imbl`) loro could adopt, and
`glib`/`gtk-rs` could move off `proc-macro-error` independently of us. Revisit
when either upstream does, or when a real vulnerability (not just
"unmaintained") lands in one of these three.
