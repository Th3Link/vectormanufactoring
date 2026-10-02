# ADR 0008: End-to-end encryption of sync and collaboration

**Status:** Accepted (customer sign-off, 2026-10-02)

Split out of ADR 0004, which assumed a server holding plaintext snapshots. The
customer's direction: *"the server will also be open source. Even though
everyone can run their own sync server, I'll also offer a hosted one. And yes,
we and anyone else hosting such a server are then the data controller. Still, we
must encrypt the data and make it cryptographically accessible to users, per the
current state of the art."* Revocation is also a requirement — *"yes, there must
be a way to remove collaborators again"* — decentralized, git-like, with
multiple admins, so option B below is **decided, not deferred**: the document
key is wrapped per participant and rotates on removal. Its mechanics are
**[ADR 0010](0010-document-keyring-admins-and-revocation.md)**.

## Context

ADR 0004 §5 put a relay server in the product: it forwards CRDT updates inside
a document room, keeps the latest snapshot so a late or reconnecting peer
converges, and stores `.vmf` projects and library records as blobs for cloud
sync. It assumed the server could read what it stores.

The requirement now is that it cannot. TLS protects the wire and disk
encryption protects a stolen drive; neither helps against the operator —
ourselves included — because in both cases plaintext exists on the server.
"Cryptographically accessible to *users*" means the decryption key exists only
on the users' machines, so the server becomes a blind relay for ciphertext.

Two things make this harder than encrypting a file:

- **Multiple writers, no coordinator.** Every peer encrypts its own updates.
  There is no server-side sequence to hang a nonce counter off and no single
  writer who owns the key material.
- **The server can no longer do the one computation ADR 0004 gave it.**
  Compacting a room's updates into a snapshot requires merging CRDT state,
  which requires plaintext. That part of §5 does not survive; Decision §6 is
  the replacement and the largest structural effect of this ADR.

This also lands squarely in the "cannot be changed cheaply" category: the
envelope around every update and every stored blob is a format we write.

### Options considered

**A. One symmetric document key, distributed out of band.** A 256-bit key
generated client-side when a document is first shared. Every CRDT update is
sealed with an AEAD before it reaches the relay, which sees only a small
cleartext routing header. The key travels in an invite — a link or QR code
whose key material sits in the URL fragment and so reaches no server — or over
any channel the users already trust. Small: one AEAD, one key, no protocol
state machine. Costs: anyone holding the invite holds the key forever; removing
one participant means rekeying the whole document; no forward secrecy; and the
invite channel is the weak link, because users paste links where they should
not.

**B. Document key wrapped to each participant's public key.** Each device
generates a long-lived keypair (ed25519 identity, X25519 agreement) and the room
stores, per participant, the document key sealed to that public key. Adding a
participant is one wrap; removing one is a new key epoch wrapped to everyone who
remains. This gives per-participant revocation and, with signatures,
cryptographic authorship inside the room. Costs: it requires ADR 0004 §10
option (ii)'s persistent device identities, which that ADR deliberately
deferred; it needs out-of-band key verification anyway, because a malicious
relay substituting a public key reads everything; and it is a rotation state
machine, which is real code with real failure modes.

**C. MLS (RFC 9420), e.g. via OpenMLS.** The correct answer to group key
agreement: epochs, forward secrecy, post-compromise security, membership
changes specified by people who do this for a living. Rejected for now on three
grounds, each sufficient: MLS presumes an authentication service vouching for
member credentials, which is an accounts system and out of scope (ADR 0007);
its delivery-service requirements are a heavier contract than "a relay that
forwards bytes", so it would reshape the server rather than fit behind it; and
it is a large dependency whose `wasm32-unknown-unknown` viability we have not
verified, while `CLAUDE.md` §6 requires the browser target to stay possible.
For two to five collaborators on a drawing it is textbook §5 over-engineering.
It stays the named destination if the product ever grows real group management.

**D. Server-side encryption at rest, server holds the key.** Rejected
outright: it is exactly what the customer ruled out. The operator can decrypt,
so it protects a stolen disk and nothing else.

**E. Password-derived key per document, no invite.** Collaborators type a
shared passphrase; Argon2id derives the key. No key transport at all. Rejected
as the primary mechanism: user-chosen passphrases are the weakest key source
available, and the server sees enough to attempt offline guessing against
captured ciphertext. Available as a manual key-entry path for users who refuse
to pass a link, since it costs one function.

### Why B

**B is chosen outright**, because the customer requires revocation and A cannot
provide it at any price: with one shared key, removing one person means rekeying
everyone — B with extra steps and no participant identities to wrap to. A's
invite UX is kept, as ADR 0010 §7's invite-link flow ending in a real
per-participant wrap rather than a bare shared secret, so the user-facing cost
of the move is zero and the engineering cost is the keyring log. The envelope
itself does not change: the first draft chose A but cut B's seams into the
format (an epoch field, and the key *source* kept separate from the sealing), so
only what sets the epoch is new.

This also settles ADR 0004 §10: B needs option (ii)'s device keypairs, so they
are no longer deferred. What ADR 0010 adds to (ii) is that authority moves out
of the server — the roster is signed, replicated data, not a record our relay
owns — which is what makes it survive folder sync, self-hosting and our server
disappearing. The room token still admits you to the room; the keyring decides
whether you can read it.

C (MLS) stays rejected on all three of its stated grounds, and nothing in
ADR 0010 is a step away from it: epochs, per-participant wraps and signed
membership are the concepts MLS formalizes, so a later migration is conceptual
rather than inventive.

## Decision

1. **The relay and the sync store never receive plaintext document content.**
   Not for collaboration updates, not for `.vmf` blobs, not for library
   records, not for thumbnails, not for the `document.json` export of ADR 0004
   §1. There is no plaintext mode, no development flag that disables encryption
   and no "internal deployment" exception (`CLAUDE.md` §5 forbids speculative
   flags, and this is the one flag most likely to be left on by accident).
   Tests use fixed test keys, not an unencrypted path.
2. **Sealing: XChaCha20-Poly1305 with a 192-bit random nonce per message**
   (`chacha20poly1305`, RustCrypto). The extended nonce is the reason for the
   choice: with many independent writers there is no counter to coordinate, and
   24 random bytes make reuse a non-issue. A 96-bit-nonce AEAD would need
   per-writer counter state that survives restarts and offline edits, which is
   precisely the bug we are not going to write.
3. **Associated data binds every message to its place:** format version,
   document id, key epoch and sender peer id are authenticated but not
   encrypted. A relay — honest, compromised or hostile — therefore cannot move
   a valid update into another document or another epoch. It can still drop,
   delay or reorder updates; CRDT operations commute, so reordering is
   harmless by construction, and withholding is denial of service, not
   corruption.
4. **Keys and nonces are parameters, never ambient.** `vecmanf-crypto-core`
   (new crate, authorized by this ADR per `CLAUDE.md` §5) holds the envelope as
   pure functions — seal, open, header parse, invite encode/decode — plus
   zeroizing key newtypes. It generates no randomness and reads no clock, so it
   builds for `wasm32-unknown-unknown` with no feature gymnastics and its tests
   are fixed vectors. Entropy comes from the OS CSPRNG in `vecmanf-storage-io`
   (and from `crypto.getRandomValues` in the browser build), passed in.
5. **The server may depend on `vecmanf-crypto-core` and on nothing else from
   core.** This is a narrow, deliberate amendment to ADR 0004 §5's "the server
   depends on no `*-core` crate": it needs the routing header parser, and
   sharing that is strictly better than a second implementation drifting out of
   agreement with the first. The *reason* behind 0004 §5 is preserved —
   `vecmanf-crypto-core` contains no document types, so a document-model change
   still cannot force a server deploy.
6. **Snapshots are produced by clients, not by the server.** The server stores
   the most recent *client-sealed* snapshot per room as an opaque blob, next to
   the sealed update log. A peer holding full state seals a snapshot and uploads
   it; a late joiner fetches that snapshot plus every sealed update after it.
   Consequences to handle rather than discover: a room nobody has opened since
   creation has no snapshot and a joiner replays the whole log; the log only
   shrinks when some client produces a newer snapshot; and a client uploading a
   corrupt snapshot is a trusted-insider failure (§8) — snapshots are signed by
   the sealing client (ADR 0010 §11), which makes such a snapshot attributable
   and one from a non-member refusable, though not detectable in content. The
   server prunes updates older than its newest snapshot only on a client's
   explicit instruction, because it cannot verify the snapshot covers them.
7. **Key custody: epoch keys and device identity keys live in the OS
   keychain**, through the same credential module as ADR 0007 §1 — including its
   encrypted-file fallback. **Local `.vmf` files are plaintext at rest, by
   decision and not by omission:** encryption here is a sync and relay
   *transport* concern, the document is decrypted to be edited, and encrypting a
   local file with a key stored on the same disk is theatre — protecting the
   local disk is the operating system's job. This is also what makes ADR 0010
   §13's worst-case recovery possible at all: the last surviving copy of a
   document is an ordinary project file, not a special backup.
8. **The document key is wrapped per participant and rotates on removal.** Each
   epoch has one symmetric document key (§2 seals content with it), sealed to
   each participant's X25519 public key; the wraps plus the roster plus admin
   roles are the signed keyring log of
   [ADR 0010](0010-document-keyring-admins-and-revocation.md). Removal is
   `RemoveMember` followed by a new epoch wrapped to everyone who remains.
   **For *content* the trust boundary is still the room:** every current
   participant can read and write everything, because they all hold the same
   epoch key — no read-only members, no per-object permissions (ADR 0010 §12).
   What per-participant keys add is membership control and attributable
   authorship: keyring entries and snapshots are signed (ADR 0010 §11), so
   "anyone in the room can claim any peer id" is a header flag away from being
   false rather than a permanent property.
9. **Cloud sync blobs are sealed with a per-user sync key**, generated on first
   use, stored in the keychain (§7), and **exported once as a recovery key the
   user must save** — words and digits, printable, shown exactly at setup with
   a confirmation that it was stored. The same recovery key covers the device
   identity keys of ADR 0010 §1, so restoring it restores the ability to be a
   participant and an admin, not only the ability to read blobs. There is no
   escrow, no reset and no recovery path through us: the design that stops us
   reading the data is the same design that stops us helping. Stated in the UI
   at setup, not in a FAQ.
10. **Metadata is not protected, and the ADR names it instead of implying
    otherwise.** The server necessarily sees room and document ids, blob ids
    and sizes, IP addresses, connection and update timing, participant counts
    per room, and therefore who is working with whom, when, and roughly how
    much. **ADR 0010 adds to this list:** per-room participant public keys and
    fingerprints, the membership graph and every change to it, because the
    keyring log is public data the relay stores, validates and forwards. That is
    a persistent pseudonymous identity per device per room — no email, no
    password, no registration, so still not an account, and strictly more than
    ADR 0004 §10 option (i) exposed. Content is opaque; activity and membership
    are not. Reducing this further (padding, cover traffic, onion routing) is
    not attempted and is not planned.
11. **The envelope's epoch field is now live, and one speculative field
    remains.** Rotation code exists (ADR 0010 §8), so the epoch is no longer a
    reserved integer — it is what the key schedule turns. The remaining
    deliberate exception to `CLAUDE.md` §5 is the per-update signature flag in
    the header, unset in MVP and justified on the same stated ground: wire and
    file formats are in the "cannot be changed cheaply" set, and one header bit
    now beats a format migration across every stored blob later.
12. **The server is published under the same licence as the client**
    (AGPL-3.0, ADR 0006) and self-hosting is a supported configuration, not a
    theoretical escape hatch: the relay URL is a setting, the protocol is this
    ADR plus 0004 §5, and a self-hoster needs no key material because there is
    none to hold. AGPL §13 applies to the hosted instance, and since the server
    is open source anyway, it costs nothing.

## Consequences

- **Under this design the operator — us or any self-hoster — is a data
  controller for traffic metadata and for account-free session data (§10), and
  not for plaintext document content.** That is a real reduction in exposure
  and it is the honest limit of it: a privacy policy, a deletion path, a breach
  process and a hosting-location decision are all still required, and a breach
  still discloses who collaborated with whom and when. "End-to-end encrypted"
  is not "we hold nothing about you", and must not be marketed as such.
- **Lost key, lost data.** No escrow (§9) means a user who loses both keychain
  and recovery key has unreadable blobs on our server that we cannot restore.
  Backups protect against our failures, not the user's. This will produce
  support requests we can only answer with no.
- **Revocation exists and is forward-only.** Removal takes effect for everything
  written afterwards (§8) and for nothing written before: the removed device
  keeps its plaintext `.vmf`, its cached updates and every epoch key it
  legitimately held. That is the standard limit of encryption rather than a
  shortcut — no scheme retracts a key someone has already used — and the
  customer accepted it explicitly. The obligation it creates is on wording:
  nothing in the UI may suggest removal reaches backwards.
- **Membership is now a second replicated, versioned structure** with its own
  file in the `.vmf`, its own relay stream, `format_version` and merge rules
  (ADR 0010 §2, §5). That is the price of revocation, paid in the hardest kind
  of code — authorization logic where a wrong tiebreak re-admits a removed
  participant.
- **Key loss is now also admin loss.** §9's recovery key covers the device
  identity keys, so losing both keychain and recovery key costs standing in
  every document the user administers, not only their blobs. A second admin per
  document is the only mitigation and the UI must push for one (ADR 0010 §13).
- **No forward secrecy, and epoch rotation does not provide it.** Participants
  retain every epoch key they held, deliberately, because the relay's stored log
  and old snapshots are sealed under those epochs and must stay readable to the
  people who were there (ADR 0010 §9). A key disclosed later still decrypts
  everything captured under that epoch. This is the single biggest gap against
  the state of the art the customer asked for, and the reason option C is named
  rather than dismissed.
- **The invite is still the attack surface, with a bounded blast radius.** An
  invite link is one-time, expiring and grants one membership (ADR 0010 §7), so
  a link pasted into a public chat costs one unwanted participant who can be
  removed, rather than permanent unrevocable read access. The UX must still make
  it feel like a key: short validity, no "copy link" without a word about what
  it grants, and the fingerprint-verified flow offered first.
- **Server-side features that need plaintext are permanently off the table:**
  search across a user's projects, server-generated thumbnails, compaction,
  format migration of stored blobs, and any web viewer that is not a full
  client. Wanting one contradicts this ADR and needs a new one — not a story.
- **Debugging gets harder in a specific way:** "collaboration diverged" cannot
  be investigated from server data. Client-side diagnostics (a local, opt-in,
  sealed op log the user can send us) are the replacement and need their own
  story.
- Recorded in `docs/technical-debt.md`: the forward-only limit of revocation,
  the remaining absence of forward secrecy, the absent key recovery, the
  all-admins-lost freeze, the keyring fork-resolution risk, the late-joiner
  dependency on a client-produced snapshot, and that `vecmanf-crypto-core` is
  code of ours in the path of every byte — now including authorization logic —
  and needs a focused review before it ships.

## Question put to the customer, and the answer

**Is end-to-end encryption part of the first collaboration story, or does an
unencrypted internal version ship first?** Put with the recommendation **not to
defer**, and accepted on that recommendation: E2EE is in the first
collaboration story, with per-participant key wrap and rotation (option B,
ADR 0010) in it rather than after it, and forward secrecy the one named
follow-up.

The reason is structural rather than principled, and it is recorded because it
is what a future reader will want: deferring duplicates the work rather than
saving it. The part of E2EE that costs real design is §6, the server losing the
ability to compact, and a non-E2EE prototype would be built around exactly the
server-side snapshot that E2EE deletes — so the throwaway version throws away
the hardest part, and the retrofit touches the wire format, the server's storage
model and the join path. Sealing bytes with an AEAD is roughly a day; rebuilding
the join path twice is not.
