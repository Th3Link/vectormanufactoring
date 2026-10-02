# ADR 0004: Persistence, collaboration and cross-machine sync

**Status:** Accepted (customer sign-off, 2026-10-02)

Supersedes the "no sync engine of ours" decision in the first draft. The
customer's direction: *"not just sync — cooperative work from the start,
similar to the Zed editor, but for vectors. Two people on two instances must be
able to work in the same file at the same time. That needs a small server. Sync
via Nextcloud for offline use is fine as an option, but there must be cloud
sync too."* Real-time collaboration is therefore a day-one requirement, not a
later story, and we are operating a server. That server is **open source with
self-hosting supported** (§13) and **cannot read what it stores**: the
encryption design is
[ADR 0008](0008-end-to-end-encryption-of-sync-and-collaboration.md), which
changes §5 here — the relay no longer computes snapshots, because it cannot see
plaintext.

## Context

Four kinds of data have to persist: **projects** (documents plus their job
setups), the **machine and material database** (per machine and material
settings plus material test results, which are expensive to produce and must
never be lost), **asset libraries** (templates and finished designs), and the
**font collection**. All four must be available on more than one of the
customer's computers.

The file formats we write are in the "cannot be changed cheaply" category.
Real-time collaboration raises the stakes on that, because a collaboration
mechanism that is chosen for the wire also tends to dictate the file.

**Two kinds of concurrency, deliberately kept apart:**

- **One document, two people, right now.** Needs sub-second convergence and a
  merge that cannot lose work. This is the new requirement.
- **Many small records, several machines, often offline.** The material
  database and library catalogs. Edits are rare, independent and almost never
  concurrent on the same record. This does *not* need the same machinery, and
  giving it the same machinery would be the classic over-engineering mistake
  (`CLAUDE.md` §5).

### Options considered — collaboration mechanism

**A. CRDT document plus a dumb relay server.** Each peer holds a replica;
edits are local, immediate and commutative; the server relays updates and keeps
the latest snapshot so a peer can join late or reconnect. Offline editing and
later reconnection are the *same* code path as live collaboration, not a
special case. This is Zed's shape and the reason the customer named Zed.
Costs: CRDT metadata grows with history (tombstones, operation ids), the CRDT's
binary format becomes a format we support forever, and a vector document's
**node tree** is the hard case — reparenting and z-order reordering under
concurrency is where naive CRDT trees produce cycles or duplicate nodes.

**B. Operational transformation with an authoritative server.** Smaller
payloads, no tombstone growth, and a single serialization point that makes
"what is the current document" trivially answerable. Rejected: OT needs a
correct transform function for *every pair* of operation types, and our
operation set is large and still growing (path node edits, transforms,
reparenting, style changes, layer/job assignment, text) — getting that wrong
silently corrupts documents. It also requires the server to be reachable to make
progress, contradicting the offline use that is explicitly wanted. OT suits a
team that can dedicate someone to it; we cannot.

**C. Lock per object, thin server.** A short lease on a node or subtree; a peer
may only edit what it holds. No merge semantics at all, so by far the least code
and the easiest to reason about. Rejected as the primary mechanism: it makes two
people editing the same path impossible rather than merged, it needs lease
timeouts and crash recovery to avoid deadlocking the document, and it cannot
work offline. It is the right mechanism for a *different* problem and is kept
for it (§4): non-mergeable singleton resources such as a machine's serial port
during a job.

**D. Shared append-only command journal with client-side rebase.** Tempting
because the first draft of ADR 0002 §9 made every edit a command with an
inverse, so the journal exists. Rejected: rebasing commands against concurrent
commands *is* OT with the same per-pair transform problem, reached by a
nicer-looking route. The journal stays in a changed role (§2) and the inverses
are gone — ADR 0002 explains why a stored inverse is a data-loss bug under
concurrency.

### Options considered — which CRDT

**Loro** and **Automerge** are both Rust, both wasm-ready, both ship a sync
protocol and a binary snapshot format, and Automerge is the more mature with the
better-documented storage format. **Loro is chosen for one specific reason: it
implements movable trees** (Kleppmann-style move operations with fractional
indexing), and a vector document is a tree whose reparent and reorder operations
are first-class user actions — "move this path into that group", "bring to
front". Automerge has no tree-move primitive, so we would build
move-without-duplication on top of it, which is precisely the part of a CRDT
that is easy to get subtly wrong. Paying Loro's lower maturity to avoid writing
the hardest CRDT code ourselves is the better trade; writing our own CRDT was
rejected outright. The maturity risk is real and is mitigated structurally, not
by hope: §3 and §9.

### Options considered — persistence and cross-machine sync

**Files only, moved by the customer's own sync tool** (Syncthing, Nextcloud).
Zero server, zero cost, offline-native — but cloud sync is required and a server
exists regardless. Kept as the *optional* path (§7).

**Our own sync service.** Unavoidable: the collaboration relay is a server and
"there must be cloud sync too" is a stated requirement. The honest change from
the first draft is that the argument against it (accounts, ops, GDPR, cost) was
not wrong — those costs are now accepted rather than avoided. See Consequences.

**One embedded SQLite database per concern, synced as a file.** Rejected as the
sync-facing shape: a binary database file is the worst thing to put in a
file-sync tool, and SQLite is I/O so it could never live in a `*-core` crate.
SQLite remains available as a local, rebuildable *index*.

## Decision

1. **A project is a single file**, a zip container with extension `.vmf`
   holding `document.loro` (the CRDT snapshot — see §3), `assets/` for embedded
   bitmaps and fonts, `jobs/` for generated job setups with their embedded
   parameter snapshots (ADR 0002), `thumbnail.png`, `document.json`, and —
   once a document has been shared — `keyring.log`, the signed membership log of
   ADR 0010 §2. The keyring sits beside `document.loro` rather than inside it,
   because a joiner must be able to read their own key wrap before they can
   decrypt anything.
   `document.json` is a **plain, non-authoritative export** of the current
   state, written on every save and **never read on a normal open**. It exists
   for diffing, debugging, scripted reading and recovery, and the one path that
   reads it is an explicit "recover from snapshot" action that starts a fresh
   CRDT history. There is exactly one source of truth at all times. The
   container is plaintext on local disk and sealed before it is uploaded
   (ADR 0008 §9) — `document.json` included, since a plaintext export is
   exactly what must not reach a server.
2. **Collaboration mechanism: a Loro CRDT replica of the document, inside
   `vecmanf-document-core`.** The command journal of ADR 0002 §9 stays the only
   way to edit a document, but its role changes: a command is now *translated
   into CRDT operations* rather than being the persisted history.
   **Consequence that must be handled, not discovered:** undo can no longer be a
   single global stack — it must be scoped to the local peer (undoing my last
   change, not my collaborator's). Loro's peer-scoped undo manager is the
   mechanism. **ADR 0002 has been revised accordingly** — §9 (the CRDT
   operation log is the canonical history) and §12 (peer-scoped undo) — so the
   two ADRs no longer contradict each other.
3. **The CRDT is an implementation detail of `vecmanf-document-core`'s API, and
   that is enforced.** No Loro type appears in the crate's public interface, in
   `vecmanf-ui-core`, in the renderer or in the frontend. The exit path if Loro
   does not hold up is then: write a new backend behind the same API and a
   one-way migration that reads old `.vmf` files through Loro and writes the new
   format. That exit is expensive but it exists, and it is the reason §3 is a
   decision rather than a guideline.
4. **Locks are used for machine resources, not for document content.** A
   serial port, a running job, a device calibration — one owner at a time,
   leased by the server, released on timeout. This is option C in its correct
   place.
5. **The server is small, knows nothing about vectors, and cannot read what it
   stores.** Rust, `axum`, WebSocket. Two jobs: (a) relay sealed CRDT updates
   within a document room and store the most recent *client-sealed* snapshot so
   late joiners and reconnecting peers converge; (b) cloud sync — store and
   serve sealed `.vmf` projects and the per-record library files of §6 as
   opaque blobs. It performs no document parsing, no merging, no geometry and
   no decryption — it could not decrypt if it wanted to, because no key reaches
   it (ADR 0008). It lives in its own crate (`vecmanf-sync-server`, authorized
   by this ADR per `CLAUDE.md` §5) and depends on no `*-core` crate except
   `vecmanf-crypto-core` for the cleartext routing header (ADR 0008 §5), so a
   document-model change still cannot force a server deploy. **Snapshot
   compaction is a client's job, not the server's** — ADR 0008 §6 — which is
   the one place where end-to-end encryption changed this decision rather than
   wrapping it.
6. **The machine/material database and asset catalogs stay one small file per
   record** — `machines/<uuid>.json`, `materials/<uuid>.json`, test results as
   an **append-only** log per machine/material pair. Every record carries a
   UUID, a `format_version` and a `modified` timestamp. No CRDT here: this shape
   already makes concurrent edits on different records conflict-free, and test
   results are only ever appended. Cloud sync and folder sync both move these
   files as files.
7. **Three sync paths, one of them required, two optional, user's choice:**
   **cloud** (our server, §5 — the default and the one that must exist),
   **folder** (point the data directory at Nextcloud/Syncthing; fully offline,
   no server involved), and **local only**. Folder sync keeps the obligations
   from the first draft: configurable data directory, atomic writes
   (write-temp-then-rename), no lock files that break in a synced folder, and
   detection plus a visible report when a sync tool leaves conflict copies
   behind. Live collaboration requires the cloud path; folder sync cannot
   provide it and must say so in the UI rather than appearing to.
8. **Merge rules for records are pure functions in core**, not in the storage
   layer: `vecmanf-library-core` owns the record model and
   `merge(local, remote) -> Resolution` as deterministic, tested code;
   `vecmanf-storage-io` does the filesystem and the network. The cloud path and
   the folder path therefore reuse the *same* merge code, which is why it was
   written this way.
9. **Format versioning from the first commit, for both formats.** Every written
   file carries `format_version`; readers accept their own version and older
   ones and migrate on load; writers always write current; a newer file is
   refused with a clear message, never partially read. For `.vmf` this covers
   the container *and* the Loro snapshot version — a Loro format change is
   treated as a format migration of ours, with golden-file tests of real old
   files, not as a dependency bump.
10. **Collaboration identity is a session identity, not an account.** See the
    dedicated section below.
11. **Asset libraries:** local folder with a manifest, plus remote providers —
    now a separate decision, **ADR 0007**, because it brings third-party
    credentials and network egress with it.
12. **Fonts** are referenced from a user-managed font collection directory, not
    copied into each project by default, with an explicit "embed fonts" option
    on export/archive. The font collection is a library in the sense of §6 and
    syncs the same way.
13. **The server is open source under the same licence as the client**
    (AGPL-3.0, ADR 0006) and **self-hosting is a supported configuration**: the
    relay URL is a setting, the protocol is this §5 plus ADR 0008, and a
    self-hoster holds no key material because there is none to hold. The hosted
    instance is a convenience, not a lock-in point — which is also what makes
    the data-controller position in Consequences tolerable, since a user who
    does not accept it has somewhere to go.

### Collaboration identity vs. user accounts — not the same thing

Collaboration needs *some* answer to "who is this peer": a name next to a
cursor, an author on a change, a participant list, and some way to decide who
may join a document room. That is a narrow, local, disposable notion. It is
**not** the accounts-and-payments system the customer has ruled out of scope
(ADR 0007: we never build accounts or payments; that is the asset providers'
job). The lead must not conflate the two, and no work on collaboration identity
may grow into user accounts without a new ADR.

Two options for the MVP:

- **(i) Room token plus ephemeral display name.** A document gets an
  unguessable room token; whoever has the token may join; each peer supplies a
  display name and gets a colour. The server stores a room id, a snapshot and
  nothing about people. No registration, no persistent identity, no personal
  data on the server. Cannot revoke one participant without rotating the token
  for everyone.
- **(ii) Local device keypair (ed25519), trust-on-first-use.** The client
  generates a keypair on first run; the server stores room id → authorized
  public keys. Gives per-participant revocation and verifiable change
  attribution. It is still not an account — there is no registration, no email,
  no password, no recovery — but it *is* a persistent identity record on our
  server and should be confirmed as such.

**Decision: both, in the shape ADR 0010 settled.** The first draft chose (i)
alone as the narrowest thing satisfying the requirement (`CLAUDE.md` §5) and
named (ii) as the next step once a story needed revocation. Revocation is now a
requirement, which is that trigger: per-participant key wrapping is the only way
to remove a collaborator and it needs exactly (ii)'s device keypairs. **(i)
survives as the join credential** — the room token admits you to the room,
unchanged — and (ii) decides what you can read, through the signed keyring of
[ADR 0010](0010-document-keyring-admins-and-revocation.md).

One correction to (ii) as described above: **the server does not hold the
authorized-key list as its own record.** The roster is a signed, replicated log
travelling in the `.vmf` and over any sync path; the relay stores and validates
it like any other stream and may refuse a non-member's connection as defense in
depth, but it is not the authority. That is what keeps membership working over
folder sync, under self-hosting, and after our server is gone. Per-room device
public keys are a persistent pseudonymous identity record wherever they are
stored; the customer was asked and accepted that (ADR 0010, answered
questions).

## Consequences

- **We now operate infrastructure, and this is the largest new cost in the
  project.** Ongoing: a host, TLS renewal, uptime and an on-call expectation the
  moment two people rely on a session, backups *and tested restores* of customer
  project data, storage growth, abuse and rate limiting, and a security-update
  treadmill on a network-facing service. None of it is big, none of it is
  optional once someone collaborates, and all of it is permanent. This changes
  what the project *is*: from a desktop application to a product with a service
  attached.
- **Legal and privacy duties follow the data, and end-to-end encryption
  narrows them without removing them.** Whoever runs an instance — us or a
  self-hoster — is a data controller for what the server necessarily sees:
  traffic metadata, room and blob ids and sizes, IP addresses, connection and
  update timing, participant counts, the account-free session data of option (i)
  and the per-room device public keys of ADR 0010. They are **not** a controller
  for plaintext document content, because no instance can decrypt it (ADR 0008
  §1, §10). A privacy policy, deletion on request, a breach process and a
  hosting-location decision are all still required; a breach still reveals who
  worked with whom and when. Backups now protect ciphertext we cannot read, so
  they guard against our failures and not against a user losing their key
  (ADR 0008 §9). AGPL-3.0 §13 bites on this server and costs nothing, since §13
  of this ADR publishes it anyway — see ADR 0006.
- **End-to-end encryption adds two crates, two wire formats and two known gaps**
  (no forward secrecy, no key recovery) and removes every server-side feature
  that would need plaintext — search, previews, compaction and a thin web viewer
  are all permanently out. Revocation is *not* among the gaps: ADR 0008 §8 and
  ADR 0010 provide it, at the price of a second replicated structure. Those are
  ADR 0008's and ADR 0010's consequences, listed there in full; what matters
  here is that "cloud sync" in this ADR means *sealed blobs we cannot read*,
  which is less product than an unencrypted service would have been.
- **The document model carries CRDT metadata.** Documents grow with edit
  history, not just content; a long-lived file needs compaction (Loro shallow
  snapshots) and that needs a story and a measured threshold. Memory per open
  document is higher than a plain arena.
- **ADR 0002 has been revised to match** (§5, §9, §12–§16): undo is peer-scoped
  and the command journal is the edit API and undo surface rather than the
  persisted history. The reconciliation also moved three things into 0002 that
  only a replicated model needs — ephemeral state kept out of the log, per-field
  merge granularity, and reads taken against a version snapshot — so the
  document model, not this ADR, owns them.
- **Offline is still a first-class case, not a degraded one** — that is the
  property CRDTs buy, and it is why option B was rejected. Editing offline on
  two machines and reconnecting converges without user intervention for
  documents. For *records* (§6) it still does not: two offline edits to the
  same material produce a conflict the user resolves with our help. Recorded in
  `docs/technical-debt.md`.
- **Loro is a young dependency in the most expensive position in the system.**
  §3 and §9 are the containment; the residual risk is a format migration we did
  not plan for. Recorded in `docs/technical-debt.md`.
- A browser build gets collaboration and cloud sync *more* easily than
  filesystem persistence — the server is reachable, the filesystem is not. The
  browser target is cloud-first, with open/save-via-download and the Origin
  Private File System as the local path.
- Per-record files mean thousands of small files for a large asset collection.
  Listing and search need a local, rebuildable index; that index is a later
  story and is never authoritative.
