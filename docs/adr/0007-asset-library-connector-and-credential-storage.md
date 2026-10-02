# ADR 0007: External asset-library connector and credential storage

**Status:** Accepted (customer sign-off, 2026-10-02)

The connector reads **and** writes: *"that's not just a read-only client — if
someone has push rights, they should also be able to upload drawings there
directly."* Write is gated by what the user's configured credential actually
grants. **Iconify** (iconify.design) is the first read target; the first push
target is a **git forge**, built as the git wire protocol rather than any one
forge's REST API (§14). For fonts, *"something we haven't found yet"* — no
provider is identified (open question 3).

## Context

The customer's direction: *"account management and payments are out of scope
for us, that's for the asset providers. There might be a link to that, but we
connect to an asset library only via API token or username/password. This is
also an essential feature regarding monetization — it's the only thing that can
make money, everything else is deliberately free."*

So the product must let a user attach third-party asset-library services and
browse, search and use their content next to local libraries, authenticating
with credentials the user obtained from the provider. We build the **client**
and nothing else.

**Upload is part of that client.** A user holding a credential with push scope —
a git forge, a WebDAV share, a studio's internal library — expects to send a
finished drawing there from the application instead of exporting to disk and
uploading by hand. That is not a new business relationship: the destination is a
service the user already has write access to, authorized by the credential they
already configured, and nothing about it moves us toward accounts or payments.

This gets its own ADR rather than a paragraph in ADR 0004 for three reasons,
each sufficient on its own:

- It is the **first network egress in the application**, and with upload the
  only path on which the user's *own work* leaves the machine to a third party.
  That is wanted — the user picks the destination and holds the credential — but
  only while every transfer is one the user asked for, which is why §10 exists.
- It stores **somebody else's secrets at rest**. Getting that wrong leaks
  credentials to paid services — a security failure with a named victim, not a
  bug.
- It is named by the customer as the **monetization-critical** feature, so the
  boundary between what we build and what providers build has to be written
  down, not inferred.

It also resolves the "accounts, payment and paid asset libraries" follow-up left
open by the first drafts of ADR 0004 and 0005 — by deciding that we build no
part of it except the connector.

### What we explicitly do not build

- **No user accounts of ours.** No registration, password reset, profile or
  entitlement database. The credential the user enters belongs to the
  provider's account system.
- **No payments, billing, subscription state or revenue share.** Purchasing
  happens at the provider, in a browser. Where a provider has a purchase page we
  may open it in the system browser — the "link to that" the customer mentions,
  a hyperlink and not an integration.
- **No client-side entitlement enforcement and no DRM.** What a token may fetch
  is the provider's server's decision. We implement no licence checks, do not
  try to stop a user copying content they downloaded, and report no usage back.
  A provider wanting enforcement must do it server-side; the application is AGPL
  (ADR 0006), so a client-side check would be removable anyway and pretending
  otherwise would mislead the provider.
- **No library hosting of ours.** Upload (§10–§12) always targets a provider the
  user chose and has write access to. We do not accept, host, moderate, index or
  distribute uploaded assets, and our sync server (ADR 0004 §5) is not an
  asset-library provider — it stores the user's own projects for the user's own
  machines. "Publish to a library" and "sync my projects" are two features with
  two servers and must not be joined.
- The provider credential is a **different thing** from the collaboration
  identity of ADR 0004 §10. Neither is an account; they must not be merged into
  one "user" concept (§15 adds a third).

### Options considered — credential storage at rest

**A. OS keychain via the `keyring` crate.** Secret Service (GNOME Keyring,
KWallet) on Linux, Credential Manager on Windows, Keychain on macOS. Secrets
never touch a file we write, access is gated by the session unlock, and other
applications cannot read them — the answer users and auditors expect. Cost: on
Linux it needs a running Secret Service, a common failure mode on minimal
window managers and headless setups, and it pulls in D-Bus. No browser
equivalent.

**B. Encrypted local file, key derived from a user passphrase** (Argon2id →
XChaCha20-Poly1305). Works everywhere including headless, portable, no OS
dependency. Costs: it is us implementing credential storage, the kind of code
where a mistake is not caught by tests, and it needs a startup passphrase
prompt, which is friction users route around.

**C. Plaintext or obfuscated file with `0600`.** Rejected. These are secrets
for services the user pays for, and "only readable by your account" is no
defence against the realistic threats (backup copies, synced folders, support
bundles, another process running as the same user).

**D. OAuth device flow / short-lived tokens only, nothing stored.** Best where
available — but the customer states providers offer API tokens or
username/password, so it is not. Kept as the preferred shape if a provider ever
supports it.

### Options considered — where the connector lives

**One new `vecmanf-library-io` crate** versus **modules inside the existing
`vecmanf-storage-io`**. `CLAUDE.md` §5 forbids a new crate without an ADR and
this is that ADR, so either is available — which makes it a KISS question, and
KISS says the second: HTTP and credential access are two *modules*, each with
one responsibility, not a new crate with one file in it. Split it out when a
second provider family makes the crate's responsibility genuinely plural.

### Options considered — how we know whether this user may upload

Push capability is **not** a property of "remote libraries" in general. It has
two independent preconditions: the provider's API must have an upload operation
at all, and *this user's credential* must carry a scope permitting it. Iconify
has neither; a git forge token may be read-only or may push. Both facts have to
be represented.

**A. Static capability table in our code, per provider.** Cheap and honest
about the protocol, but it says nothing about the user's credential, so the UI
would offer upload to a user whose token cannot do it.

**B. Probe the provider at connect time.** Many APIs expose a token's granted
scopes (a `/user` or introspection endpoint, a WebDAV `OPTIONS` response, a
repository permission field). Where that exists it answers the real question
before the user has produced anything to upload; where it does not, there is
nothing to probe.

**C. Attempt and report.** Offer upload always, surface the provider's 401/403
clearly. Never wrong, but it fails after the user has committed to an action —
the worst moment to find out.

**Decision: A and B together, with C as the floor.** The protocol declares
whether an upload operation exists (A); at connect time we read the granted
scope where the provider exposes it (B); where neither settles it, upload is
offered and a rejection reported plainly (C). Capability is recorded per
*connection*, not per provider, and re-checked when the credential changes.

### Options considered — what "git forge" means concretely

The customer named a git forge as the first push target and did not name a host.
That is the decisive fact: the options differ in whether we build against *git*
or against *a forge*.

**A. Host-agnostic git wire protocol (`gitoxide`).** The connection is a git
remote URL plus an HTTPS token; browsing reads the repository tree, uploading
commits into a local clone and pushes. One implementation serves GitHub,
GitLab, Gitea, Forgejo and a bare self-hosted remote, because they all speak the
same protocol — and read and write are the *same* protocol, so one connector
gives both `AssetSource` and `AssetSink`. Costs: a local clone per connection,
so §8's cache grows a working copy; push is the youngest part of `gitoxide`; a
fetch-commit-push is a session rather than a request, so §5's pure/impure line
has to be drawn differently (§14); and nothing forge-specific is available — no
pull requests, no release assets, no server-side search.

**B. One forge's REST API first (GitHub's Contents API).** One HTTP request
creates a commit containing one file. The cheapest thing to build: no git
library, no clone, fits §5 exactly, and GitHub exposes token scopes and a
repository `permissions` field — precisely what capability option B wants at
connect time. Why it loses: it is one vendor. GitLab's API is shaped
differently and Gitea's is GitHub-shaped but not identical, so the next forge
means a third and fourth implementation; base64-in-JSON caps file size well
below the protocol's; and a customer who wants a self-hostable sync server is
not obviously a GitHub-only user, so this is an implementation we would rebuild
immediately.

**C. Shell out to the `git` binary.** Rejected: it makes the user's PATH part of
our protocol, has no sane error surface (parsing human-readable git output),
would pass credentials through a helper or a child environment, and does not
exist on a stock Windows install. `git2`/libgit2 is the better form of the same
idea and is §14's fallback if `gitoxide`'s push does not hold up — at the cost
of a C dependency, no wasm, and an ADR for the FFI.

**Decision: A.** The customer said "git forge", not a forge's name, and A is the
only option that honours that literally. It also buys what a vendor API cannot:
an asset library that *is* a git repository — versioned templates, a history per
reusable part, branches — which matches how the customer already works. B is
demoted rather than discarded: a forge's API becomes an **additional,
per-connection capability** for what the wire protocol cannot express (opening a
pull request, publishing a release), built when a story asks, never as a
precondition.

## Decision

1. **Credential storage: option A with option B as an explicit, visible
   fallback.** The OS keychain is used wherever one is available. Where none is
   (typically Linux without a Secret Service), the application **tells the user
   so and offers** the encrypted-file store, which requires the user to set a
   passphrase. It never silently downgrades, and there is no third option. The
   `0600` plaintext store (C) is not implemented at all.
2. **Prefer tokens over passwords, and never store a password we can avoid
   storing.** If the provider supports an API token, the UI asks for a token
   and nothing else. If the provider only has username/password, the password
   is exchanged for a session token at login and **only the token is stored**;
   the password is stored only when the provider has no token concept at all,
   and that case is labelled as such in the UI.
3. **Credentials are machine-local and are never synced.** They do not enter
   the document, the `.vmf` project file, the per-record library files, folder
   sync, or our cloud sync (ADR 0004). Replicating a user's paid-service
   credentials to our own server is a liability we decline to create. Moving to
   a new machine means re-entering the credential; that is the correct amount of
   friction.
4. **Secret hygiene is a written rule, enforced in review.** Credentials are
   held in `zeroize`-wrapped types, never in a `String` that outlives the
   request; they are redacted in all log output, error messages, panic
   payloads and support bundles; no credential ever appears in a URL or query
   string, only in an `Authorization` header; and no test fixture contains a
   real one. A log line or error that could carry a secret is a review
   finding.
5. **The core/platform split holds and is what makes this testable.**
   `vecmanf-library-core` (pure, wasm-compatible) owns the asset and catalog
   record model, search and filtering, and the **protocol as pure functions**:
   build a request description — upload requests included — parse a response,
   map provider errors to our error type. An upload is therefore a described
   request a test can assert byte for byte before any network code runs.
   `vecmanf-storage-io` owns the two impure modules and nothing else: HTTP
   (`reqwest` with TLS, timeouts, retry with backoff) and credentials
   (`keyring`, or the encrypted file). `CLAUDE.md` §5's golden-file rule applies
   to provider responses as it does to file formats, so a protocol is tested
   with recorded fixtures and no network.
6. **Two traits, each with two real implementations: `AssetSource` for reading
   and `AssetSink` for writing.** `AssetSource` is minimal — list, search, fetch
   an item, report capabilities — implemented by local-folder libraries
   (ADR 0004) and by remote providers, which the asset browser shows in one
   list. `AssetSink` is `put_item` plus its own capability report, implemented by
   a local-folder library and by a push-capable connector. Both traits therefore
   have two implementations the day the first remote connector exists
   (`CLAUDE.md` §5). **Write is a separate trait rather than optional methods on
   `AssetSource`**: a read-only provider does not implement `AssetSink` at all,
   so "cannot upload here" is carried by the type system and read off a
   capability report, not discovered as a runtime `Unsupported` error. No
   provider-specific extension points on either trait until a story needs them.
7. **Connectors are not plugins** (ADR 0005 §7). They need network access and
   credential storage, which the plugin sandbox exists specifically to withhold.
   A new provider is, for now, code in this repository. Third-party connectors
   would require the plugin capability-grant ADR *and* a user consent surface —
   and this is where a malicious plugin would go hunting for tokens, so it is a
   deliberate decision, never a convenience.
8. **Content fetched from a provider is cached locally** in the library data
   directory with its provider, item id and fetch time recorded, and is usable
   offline once fetched. Cache entries carry no credentials. Whether a provider
   permits caching is a licence question the provider's terms answer; the cache
   is per-user and local, and we do not redistribute it.
9. **A provider's terms and the content licence are shown at connect time and
   recorded with each imported asset**, because an asset bought under one licence
   and reused in a sold product is the user's legal exposure, and the
   application is the only place that knows where the file came from. **The
   licence is recorded per item, not per provider**: an aggregator is the
   realistic case — Iconify carries many icon sets under different licences
   (MIT, Apache-2.0, CC-BY, and sets that are not free) — so "which licence does
   this provider use" has no answer. An item whose licence the provider does not
   state is recorded as *unknown* and labelled, never silently treated as free.
   **Against a git remote** there is no per-item licence field, so it resolves in
   this order, with the source of the answer shown next to it: the item's entry
   in the library manifest of ADR 0004 §6; an SPDX identifier in a REUSE-style
   `<item>.license` sidecar or the file's own header; the repository's `LICENSE`,
   labelled as a **repository-wide default** and never as a per-item fact;
   otherwise *unknown*. On upload, the licence the user asserts is written into
   that item's manifest entry **in the same commit**, so the answer travels with
   the asset and round-trips on the next read.
10. **Every upload is an explicit, visible, per-item user action.** No background
    push, no "mirror this folder", no publish-on-save, no retry loop that
    uploads later unasked, and no setting that turns any of those on — §1's
    refusal to downgrade silently, applied to egress. Concretely: the user
    invokes upload on a named target; before any byte leaves, a confirmation
    names the destination provider, the path or collection, the credential, the
    exact file and format, and its size; the transfer shows progress and its
    result. One confirmation authorizes one upload; re-uploading a changed file
    is a new action. This is a **requirement on the API, not only the UI**:
    `AssetSink::put_item` takes a value only the confirmation step can
    construct, so no caller — including a future plugin or script surface — can
    push without going through it. **Against a git remote one confirmation
    equals one commit and one push**, and the confirmation additionally names
    the remote URL, the branch and whether it is the remote's default branch,
    the path inside the repository, the commit message and the commit author
    identity (§15). **Never a force-push and never an amend:** a rejected
    non-fast-forward push is reported as a conflict the user resolves in their
    own git tooling. After a push to a side branch we may offer the forge's
    compare page in the system browser — a hyperlink, like the purchase-page
    link above — but we do not open pull requests, which is a forge API
    operation the wire protocol does not have (§14).
11. **What gets uploaded is an export by default, never the project file by
    accident.** A `.vmf` contains the CRDT history, embedded assets, job setups
    with machine parameters and collaborator display names (ADR 0004 §1) —
    considerably more than the drawing. Upload therefore offers the chosen
    export format (SVG, PNG, or a machine format) as the default, and uploading
    the `.vmf` itself is a separate, labelled choice that states what it
    includes. The upload path reuses the normal export pipeline; it does not get
    a second serializer.
12. **A credential is optional.** Iconify needs none, and that must not be a
    special case bolted on later: a connection has zero or one credential, and
    an anonymous connection is a first-class state. A connection with no
    credential is read-only by construction.
13. **Upload asserts rights the user must have.** The confirmation of §10
    states that the user is responsible for holding the rights to what they
    publish, and the provider's own upload terms are shown at that point where
    the provider states them. We do not scan, filter or moderate content, and we
    do not implement takedown — there is nothing of ours to take it down from
    (see the non-goal above).
14. **The first remote `AssetSink` is a host-agnostic git remote, via
    `gitoxide`** (option A above; MIT/Apache-2.0, so ADR 0006's allow-list is
    satisfied). A connection is a remote URL, a branch, a path prefix and an
    optional HTTPS token; the same connection implements `AssetSource` by
    reading the tree at that prefix. §5's split is drawn one step further out
    than for an HTTP provider, because a fetch-commit-push is a session rather
    than a request: `vecmanf-library-core` owns what is pure — resolving the
    target path, building the manifest entry and commit message, deciding which
    blobs a commit must contain, mapping git errors — and `vecmanf-storage-io`
    owns the clone, the object store and the wire. Golden-file tests run against
    a **local bare repository fixture**, a real git remote needing no network.
    The clone is part of §8's cache; the fallback if `gitoxide`'s push proves
    unready is `git2`, which needs its own ADR for the C dependency and is not
    assumed.
15. **The commit author is a third identity and is kept separate.** A commit's
    name and email become permanent public repository history and are personal
    data leaving the machine. The author identity is therefore configured **per
    connection**, never defaulted from the collaboration display name of
    ADR 0004 §10 or anything else in the application, and shown in §10's
    confirmation before the first push. Three identities now exist —
    collaboration display name, provider credential, commit author — and they
    are not merged into one "user".
16. **A token never enters a git config.** The remote URL stored in the clone is
    credential-free; the token is supplied per operation from the credential
    store of §1 through a callback, and never written to `.git/config`, a
    `.netrc`, a credential helper, the environment of a child process, or a log
    line. This is §4's hygiene rule applied to the one place where a library
    would otherwise persist the secret for us.

## Consequences

- The monetization-critical path exists and stays narrow: a token field, an
  HTTP client, two small traits and a cache. There is no accounts system, no
  billing, no entitlement service, and therefore none of their operating cost,
  liability or GDPR surface. We cannot capture revenue directly from asset
  sales either — see the open question below.
- **Upload is user-intended egress, and the design keeps it that way.** The
  user's work leaves the machine only to a destination they named, with a
  credential they supplied, after a confirmation that says what is being sent.
  That is the difference between a feature and an exfiltration path; §10's
  API-level constraint exists so the property survives the first caller who did
  not read this ADR.
- **The first connector exercises neither the credential store nor the upload
  path.** Iconify needs no credential and accepts no pushes, so the keychain
  work (§1–§4) and `AssetSink` (§6) get their first real test from the *second*
  connector. Iconify's API is public and documented, so the read path can be
  pinned with golden fixtures immediately; the push target is testable with no
  network and no account at all, since a bare repository in a temp directory is
  a real git remote. §6's local-folder sink is still `AssetSink`'s first
  implementation — because it is smaller, not because the remote one lacks
  evidence.
- **Whether a credential may push cannot always be known in advance.** Where a
  provider exposes token scopes we check at connect time; where it does not, the
  user learns from a rejected upload. Recorded in `docs/technical-debt.md`.
- **We hold third-party secrets, and that is a new class of risk for this
  project.** Keychain first, token over password, never synced, redaction
  everywhere is the mitigation. Recorded in `docs/technical-debt.md`, where the
  unresolved part is named: the encrypted-file fallback is our own crypto-using
  code and needs a focused review before it ships.
- **On Linux without a Secret Service the user hits a passphrase prompt**, and
  Linux is the customer's primary platform. This will look like a defect unless
  the message explains it. A UX story owns that wording.
- **A git-backed library is a feature, not only a sink.** Because read and write
  are the same protocol (§14), pointing a connection at a repository of templates
  gives versioned assets, a history per item and branches with no extra
  machinery. That is the main reason option A beat the easier option B.
- **We take on a git implementation, in a position we cannot easily leave.**
  `gitoxide` is young in exactly the half we need most (push), and the fallback
  is a C library. Recorded in `docs/technical-debt.md` with a measurement to do
  before the story: push to a real Gitea and a real GitHub remote with a token,
  over HTTPS, from all three desktop platforms.
- **A clone is bigger than a cache.** §8's cache becomes a working copy per
  connection: disk growth, a repository a rejected push can leave conflicted,
  and a directory the user may also open in their own git tooling. The sink must
  tolerate a clone someone else has touched rather than assume it owns it.
- **No pull requests, no release assets, no forge search.** Those are forge API
  features option A does not have; we offer a push to a side branch plus a link
  to the compare page (§10). "Submit as a pull request" is option B as an
  additional capability and needs a story, not a redesign.
- Because the application is AGPL (ADR 0006), our connector code is public, so
  providers must not rely on connector secrecy — stated in the non-goals rather
  than left for them to discover.
- A provider offering only username/password forces us to store a password.
  Isolated to one code path and flagged in the UI, but not designable away from
  our side.

## Open questions

None of these blocks the decisions above; each has a working default that the
ADR was accepted with. They are product questions, tracked in the ADR index's
open follow-ups.

1. **The first push target is answered — a git forge**, taken as forges in
   general rather than one vendor (§14). Two sub-questions remain, both with a
   working default:
   - **Commit and push, or a pull request?** §10 does commit-and-push with a
     link to the compare page; a real PR flow needs a forge-specific API
     (option B) and its own story. Default: commit and push.
   - **HTTPS token or SSH key?** §16 handles a token, which fits §1–§4
     unchanged. SSH would mean storing a private key or depending on a running
     `ssh-agent`. Default: HTTPS token.
2. **Does "monetization" mean the customer intends to *be* an asset
   provider?** Selling one's own library through this connector is a materially
   larger scope than building the client: it means a provider service with
   accounts and payments — the very thing ruled out here — just on the other
   side of the wire. Default assumption: no; we build the client only, and the
   customer sells through an existing platform.
3. **Fonts have no source.** The customer states no suitable font-asset service
   is known. This ADR does not try to solve it and nothing here blocks on it: a
   font provider, if one ever appears, is one more `AssetSource` and needs no
   new decision. Until then the font collection is the local, user-managed
   directory of ADR 0004 §12. Recorded as an open product question, not an
   architecture one — the product owner's to carry, not this ADR's.
