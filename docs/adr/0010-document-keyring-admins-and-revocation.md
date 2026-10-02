# ADR 0010: Document keyring — participants, admins and key revocation

**Status:** Accepted (customer sign-off, 2026-10-02)

Split out of ADR 0008, whose §8 *decides* per-participant key wrapping; this ADR
is how it works.

The customer's direction, translated: *"Yes, there must be a way to remove
collaborators again… it's fine if the outcome is simply that the removed person
can no longer collaborate — they get to keep the last state they had."* And on
the worst case: *"If keyring and recovery key are both lost, nothing can be
done. But it should be a decentralized, git-like system — if there's still an
'admin', members can be re-added. Only if all admins have lost their keys does
the last copy remain, as an unencrypted copy sitting on one of the clients'
filesystems."*

## Context

ADR 0008's first draft chose option A — one shared symmetric document key in an
invite — and recorded "no participant revocation" as known debt, with option B
(per-participant key wrap) as the named destination. Revocation is now a
requirement, constrained in *how* it may be built: no central authority,
git-like, multiple admins, and a stated worst case. Restated as requirements
this ADR must satisfy:

- **R1 — Removal.** An admin can remove a participant. The removed participant
  cannot decrypt anything produced after the removal. Nothing retroactively
  strips what they already hold locally; the customer accepts this explicitly.
- **R2 — Decentralized.** Membership is not a record on our server that our
  server adjudicates. It must work over folder sync and over a self-hosted
  relay, and it must travel inside the project file.
- **R3 — Multiple admins.** Admin-ness is held by more than one participant and
  can be granted and taken away. While one admin survives, membership is fully
  repairable.
- **R4 — Worst case is named, not avoided.** If all admins lose their keys,
  there is no network path back. The surviving path is the plaintext project
  file on a client's disk.

The customer asked for the flows before the mechanism, and the product owner
wrote them up as **R-COLLAB-007 to R-COLLAB-013** in `docs/requirements.md`
§12: invite, accept, leave voluntarily, remove, re-add, multiple admins, and
the all-admins-lost limit. This ADR answers against that list — invite and
accept are §7, leave, remove and re-add are §8, multiple admins are §3 and §5,
the hard limit is §13. The one place the requirements and a naive reading of the
mechanism diverge is voluntary leaving, which §8 calls out because it is a
roster act and not a cryptographic one.

### What makes this hard: authorization is not convergence

A CRDT merges; it does not adjudicate. Last-writer-wins is the wrong rule for
"is this person allowed to be here": a removed admin's concurrent write merges
as validly as a legitimate one and may win the register. Authorization needs
*causal* evaluation — was the author an admin at the point in history they acted
from? — which is what a hash-linked history gives and a convergent register
throws away.

A second, harder constraint: **the structure that tells a joiner how to obtain
the document key cannot itself be encrypted with that key.**

### Options considered — how membership and admin-ness are represented

**A. A container inside the document's own Loro CRDT.** Membership as a map in
the replicated document: one entry per participant with role and wrapped key,
signed by the admin who wrote it. Attractive because there is one replication
path and nothing new on the wire. Rejected on two counts, each fatal: it is
sealed under the document key, so a joiner cannot read their own wrap out of it
(the chicken-and-egg above); and CRDT merge rules cannot express "this write was
not authorized when it was made" — a revoked admin's concurrent entry converges
like any other, so we would be re-deriving authority on top of a structure
designed to have none.

**B. A separate append-only, hash-linked log of signed entries, replicated
alongside the document but not sealed.** Each entry names its parents, its
author's public key, its payload and a signature; state is a deterministic
replay from genesis; an entry is valid only if its author was an admin in the
state computed from that entry's *ancestors*. It carries no secrets — only
public keys and ciphertext wraps — so it needs no encryption and can be read
before you hold anything. This is a git history with signed commits and a replay
rule, which is literally what the customer asked for. Costs: a second
replicated structure with its own file in the `.vmf`, its own relay stream and
its own merge rules, and concurrent admin action produces forks we must resolve
deterministically — real code with real failure modes, bought deliberately.

**C. The relay holds the roster and enforces it.** Smallest possible client: the
server stores room id → authorized public keys and admins call an API to change
it. Rejected: it contradicts R2 outright, makes the hosted instance one
authority and a self-hoster another, does not work over folder sync or in a
`.vmf` carried on a stick, and would put membership — the one thing that must
survive our server disappearing — inside our server. It also buys nothing
cryptographically, since the server holds no keys to wrap with.

**Decision: B.** C fails the customer's constraint, A fails on mechanics. B is
chosen with its cost acknowledged: §5's fork-resolution rules are the part of
this ADR most likely to contain a bug, so they are written here rather than
discovered in code.

## Decision

1. **Every device holds two long-lived keypairs**, generated on first run: an
   **ed25519** signing key (the participant identity) and an **X25519**
   agreement key (the wrap target). The pair of public keys, plus a short
   human-readable fingerprint, is the participant's identity everywhere in this
   ADR. Private keys live in the credential store of ADR 0007 §1 — OS keychain,
   encrypted-file fallback — and are covered by the one-time recovery key of
   ADR 0008 §9. **A participant is a device, not a person**: a second machine
   is a second participant an admin adds, shown in the participant list under a
   user-chosen label. Cheaper than a person-to-device mapping, and there is no
   accounts system to hang one on.
2. **The document keyring is a hash-linked log of signed entries**, stored as
   `keyring.log` in the `.vmf` container (ADR 0004 §1) — **not** inside
   `document.loro` — and relayed as its own stream in a document room. An entry
   is `{ parents: [hash], author: ed25519_pub, payload, signature }`; its hash
   covers everything but the signature. It contains no plaintext document
   content and no secret, so it is not sealed (ADR 0008 §1 is unaffected: there
   is nothing in it to read).
3. **Payloads, and that is the whole vocabulary:** `Genesis { document_id,
   admins, epoch0_wraps }`, `AddMember { identity, role, wraps }`,
   `RemoveMember { identity }`, `SetRole { identity, role }`,
   `NewEpoch { epoch, wraps }`, `AddPendingInvite { invite_id, role, expires }`,
   `ClaimInvite { invite_id, identity, proof }`, `RewrapEpoch { epoch,
   identity, wrap }`. No other payload type is added without an ADR, because
   each one is a new authorization rule.
4. **Validity is causal and is a pure function.** An entry is valid iff its
   signature verifies under its author key and its author holds the required
   role in the state replayed from that entry's ancestors alone — admin for
   everything in §3, with three exceptions: `ClaimInvite` is authorized by the
   matching `AddPendingInvite` plus proof of the invite secret; `RewrapEpoch` by
   any current member who holds that epoch (§9); and `RemoveMember` **also** by
   the subject themselves, which is how R-COLLAB-009's voluntary leave is
   expressed — nobody needs an admin's permission to stop being a collaborator.
   Invalid entries are dropped,
   not merged — never "applied with a warning". Replay is in
   `vecmanf-crypto-core`: no clock, no I/O, fixed test vectors, so it builds for
   `wasm32-unknown-unknown` and both client and server run the same code.
5. **Forks resolve by three rules, in this order, and nothing else.** Concurrent
   admin action is normal, not exceptional, and the merged state is a function
   of the entry set, never of arrival order:
   - **Deny wins.** For the same identity, removal beats addition, and the lower
     role beats the higher one. If one admin removes someone while another
     promotes them, they are out. Security conflicts fail closed.
   - **The admin set never becomes empty.** If a merge would leave no admin —
     two admins concurrently removing each other is the realistic case — the
     removals that caused it are dropped, the admin set stands, and the UI
     reports it. A deterministic rule that refuses to brick the document beats a
     tidier rule that can.
   - **Highest epoch wins, ties by lowest entry hash.** Two admins rotating
     concurrently produce two `NewEpoch` entries; the survivor is the current
     epoch for new writes. Keys from the losing epoch are *not* discarded (§9),
     so nothing sealed under it becomes unreadable, and a participant missing the
     survivor's wrap is repaired by `RewrapEpoch`.
6. **Key hierarchy.** Each epoch has a random 256-bit document key, generated
   client-side. A *wrap* is that key sealed to a recipient's X25519 public key
   with an ephemeral-sender ECDH plus HKDF plus the XChaCha20-Poly1305 of
   ADR 0008 §2 — no new primitive, no new dependency. The wrap's associated data
   binds document id, epoch and recipient public key, so a wrap cannot be
   transplanted to another document, epoch or recipient. Document content is
   sealed exactly as ADR 0008 §2–§3 already specify; the epoch field that §11 of
   that ADR reserved is now the thing that moves.
7. **Adding a participant: two flows, both ending in a real per-participant
   wrap.**
   - **Verified (preferred, and what the UI should nudge toward):** the invitee
     generates their keypair and sends the admin their public keys and
     fingerprint over a channel the two already trust; the admin compares the
     fingerprint and writes `AddMember` with the wraps. No secret ever transits
     a channel.
   - **Invite link (kept, because the first is the one real users skip):** the
     admin writes `AddPendingInvite` with the hash of a one-time secret and an
     expiry, and hands over a link whose secret sits in the URL fragment —
     ADR 0008's option-A invite UX, unchanged for the user. The invitee's client
     binds its own long-term keys by writing `ClaimInvite` with proof of the
     secret, and derives the epoch key from the invite. The link grants exactly
     one membership, once, until it expires.
   A relay that substitutes a public key in the verified flow is defeated by the
   fingerprint comparison and by nothing else; this is stated in ADR 0008
   option B and remains true. Both flows satisfy R-COLLAB-007's *"without either
   party handling raw cryptographic key material by hand"*: the verified flow
   asks the two people to **compare a fingerprint**, which is reading a short
   string, and the link flow asks for nothing at all.
8. **Removing a participant is removal plus rotation, as one user action.** An
   admin writes `RemoveMember`, then immediately a `NewEpoch` whose wraps cover
   every remaining participant and not the removed one. The removed device holds
   no wrap for epoch *n+1*, so every update and snapshot written afterwards is
   opaque to it. The client then seals a fresh snapshot under the new epoch and
   instructs the relay to prune older updates (ADR 0008 §6) — hygiene that
   shortens the window, not a guarantee, since the removed peer may already have
   fetched everything. **What removal does not do:** reach into the removed
   device's disk, invalidate the plaintext `.vmf` they hold, or make earlier
   history unreadable to them. The customer accepted this; the UI must say it in
   one sentence at the moment of removal.
   **Leaving voluntarily (R-COLLAB-009) is not the same operation and must not
   be presented as if it were.** A member can write their own `RemoveMember`
   (§4) and their client stops syncing, but they still hold the current epoch
   key — only an admin's rotation takes it out of use. So leaving is a
   client-side and roster-level act; if the document must become unreadable to
   the person who left, an admin rotates. The UI tells the remaining admins a
   rotation is pending, and does not tell the leaver they lost access they in
   fact still have.
   **Re-adding a removed collaborator (R-COLLAB-011)** is an ordinary
   `AddMember` plus §9's history choice — the admin decides whether the
   returning member gets the epochs from the period they were out.
9. **Old epoch keys are kept, deliberately.** A participant retains every epoch
   key it has ever legitimately held, because the relay's log and old snapshots
   are sealed under those epochs and must stay readable to the people who were
   there. Consequences, both intended: there is still no forward secrecy
   (ADR 0008 names this), and **a new member sees history only as far back as
   the epochs they were wrapped for.** An admin adding a member therefore
   chooses between *full history* (wrap every epoch the admin holds — the
   default, because otherwise the joiner cannot replay the log) and *from now
   on* (wrap the current epoch only, and seal a fresh snapshot for them to start
   from). `RewrapEpoch` exists to repair a participant who is missing an epoch
   key they are entitled to; it is safe for any member to issue because it can
   only hand a current member a key the roster already grants them.
10. **The relay stays blind and gains one cheap check.** It stores and relays
    `keyring.log` like any other stream and validates what it relays with the §4
    replay function — the log is public data, so this needs no key. It then
    admits a connection to a room only if the peer proves possession of an
    identity key the log it holds lists as a current member. This is **defense
    in depth and not the security boundary** — the server's view may be stale, a
    self-hoster may skip it, and the real guarantee is that a revoked peer has
    no key — but it cheaply buys three things: a removed peer stops occupying a
    room and accumulating ciphertext, abuse handling gets something to
    rate-limit, and forged keyring entries stop being free amplification. It
    buys no plaintext; nothing here touches content.
11. **Signatures: keyring entries and snapshots always, individual updates by a
    header flag.** Keyring entries are signed by construction (§2).
    **Snapshots are signed** by the client that sealed them — the smallest
    useful answer to ADR 0008 §6's trusted-insider corrupt-snapshot failure: a
    bad snapshot is at least attributable, and one from a non-member is refused.
    Individual updates carry a signature only when the envelope's header flag
    says so, and MVP does not set it, because 64 bytes per fine-grained
    operation is a real cost on a drag. The flag is in the format now for
    0008 §11's stated reason — formats are in the cannot-change-cheaply set —
    and it turns ADR 0008 §8's "anyone in the room can claim any peer id" from a
    permanent property into a flag flip. No signing *code* beyond §2 and
    snapshots until a story needs it.
12. **Two roles, admin and member, and no third.** An admin may add, remove,
    promote, demote and rotate; a member may edit and may `RewrapEpoch`. **A
    read-only collaborator is not offered, because it cannot be enforced:**
    everyone who can decrypt holds the epoch key, and a key that opens the
    document also seals writes to it, so the role would be a lie in the UI. If
    read-only access is ever required it is a different mechanism (export, or a
    viewer that is not a room member) and a new ADR.
13. **The worst case is a designed path, not a gap.** In order of severity:
    - *A member loses their device.* An admin removes the lost identity and adds
      the replacement. Normal operation.
    - *An admin loses their device but has the recovery key* (ADR 0008 §9). The
      recovery key restores the credential store, and with it the identity keys
      and epoch keys. Nothing else is needed.
    - *An admin loses device and recovery key, other admins survive.* The
      surviving admin removes the lost identity and adds the returning one. This
      is R3, and it is the reason multiple admins are worth their complexity:
      **the second admin is the backup.** The UI should push for a second admin
      at the moment a document is first shared.
    - *All admins have lost their keys.* No new epoch and no new member can ever
      be authorized for this document. Existing members keep working — they hold
      the current epoch key — so the document is not lost, only frozen in
      membership. There is **no network recovery path, by design**: anything that
      could re-grant admin without an admin key would also let our server, or a
      relay operator, do it.
    - *Nothing left but the file.* Every `.vmf` on every client's disk is
      **plaintext at rest** (ADR 0008 §7, ADR 0004 §1), so the last copy is
      somebody's ordinary project file with no special backup mechanism
      involved. The path back is to copy it off the machine by hand and **fork
      it**: open it, write a fresh `Genesis` with the opener as admin, get a new
      document id and a new epoch-0 key, and re-share. Other peers must accept
      the new share and the old room is abandoned. This is `git clone` into a
      new history; it grants nobody anything they did not already have, since
      they held the plaintext file. It is also the end of the line — if no
      client holds a copy and the keys are gone, the sealed blobs on the relay
      are unrecoverable by anyone, us included.
14. **Where the code lives.** Entry encoding, hashing, signature verification,
    the §4 replay and the §5 merge rules go in `vecmanf-crypto-core` as pure
    functions over byte slices — no clock, no randomness, no I/O — which is what
    lets the server share them (ADR 0008 §5's exception already permits exactly
    this dependency and no other). Key generation, keychain access and the
    network are `vecmanf-storage-io`. Expiry checks on invites take the time as
    a parameter.

## Consequences

- **Revocation prevents future access and undoes nothing.** The removed
  participant keeps every byte they already received, including the plaintext
  `.vmf` on their disk. That is not a weakness of this design but the standard
  limit of every encryption scheme, and it is what the customer agreed to. Any
  UI wording implying otherwise ("access revoked", bare) is a defect. Recorded
  in `docs/technical-debt.md`.
- **ADR 0004 §10 moves from option (i) to a keyring-backed option (ii), with
  authority moved out of the server.** Device keypairs are now required, so that
  ADR's deferral ends. The difference from (ii) as 0004 framed it matters: the
  relay holds the keyring log as replicated data it validates, not a roster it
  owns, so folder sync and self-hosting get identical semantics and our instance
  is not special. The room token survives as the join credential; the keyring
  decides who can read.
- **The relay sees more metadata, and ADR 0008 §10 is amended to say so.**
  Per-room public keys, fingerprints and the membership history are visible to
  any relay operator — a persistent pseudonymous identity per device per room,
  with no email, password or registration behind it, and strictly more than
  option (i) exposed. Confirmed by the customer rather than slipped in.
- **Fork resolution is this ADR's risk concentration.** §5's three rules are
  where a bug becomes a security bug: a wrong tiebreak re-admits a removed
  participant. They are pure functions over an entry set, so exhaustive tests on
  small membership graphs are the mitigation. Recorded as debt with a required
  review.
- **A second replicated structure means a second thing to version.** The keyring
  log gets its own `format_version` under ADR 0004 §9, its own golden-file tests
  and its own migration story, and it is one more thing folder sync must move
  atomically.
- **The UI gains a membership surface it did not have:** participant list with
  fingerprints and roles, add and remove flows, a "verify this fingerprint"
  step, the full-history-or-not choice when adding, the one-sentence truth at
  removal, and the nudge toward a second admin. A design-system story, and not
  a small one.
- **Server-side membership enforcement is optional and must stay optional.** A
  self-hoster stripping §10's check loses defense in depth and nothing else. If
  anything in the product ever *depends* on that check, this ADR has been
  violated.

## Questions put to the customer, and the answers

Both were put with a recommendation and a stated default, and both defaults were
accepted without objection:

1. **A persistent per-room device identity is acceptable.** It is the direct
   consequence of requiring revocation — there is nothing to wrap a key to
   without it. ADR 0007's hard boundary stays intact (no accounts, no email, no
   registration, no server-side recovery) and the added metadata is stated in
   the privacy policy.
2. **A participant is a device, not a person** (§1). The device model needs no
   accounts; a person model would need device linking, which is a key-hierarchy
   decision and a UI. The participant list carries a user-chosen label, so
   "Marc's laptop" and "Marc's workshop PC" are legible side by side. Revisit —
   in a new ADR — only if work across more than two or three machines makes the
   device model awkward.
