# ADR 0009: Concurrent editing semantics — undo, ephemeral state and merge granularity

**Status:** Accepted (customer sign-off, 2026-10-02)

Split out of [ADR 0002](0002-document-model-units-and-svg-round-trip.md) while
reconciling it with [ADR 0004](0004-persistence-and-cross-machine-sync.md), for
the same reason ADR 0008 was split out of 0004: these are decisions with their
own options and their own permanent, *user-visible* consequences, and 0002 is
already at the limit of what can be read in five minutes. 0002 owns the shape of
the document; this ADR owns what happens when two peers change that shape at the
same time. §1 changes what Ctrl+Z means, so it was put to the customer with a
recommendation and a stated default, and accepted on that default (ADR index,
customer sign-off).

## Context

ADR 0004 §2 makes an open document a Loro CRDT replica; ADR 0002 §9 makes the
operation log its canonical history and the command journal a translation and
presentation layer. That settles where edits are *recorded*. It does not settle
three things that the user can see directly:

- **What undo does** when the document has more than one author. The first
  draft of 0002 promised a single global stack of invertible commands, which a
  CRDT cannot deliver and should not fake.
- **What is an edit at all.** A node drag produces hundreds of input events. If
  each is an operation, the log and every collaborator's screen drown in them;
  if only the final state is an operation, collaborators see nothing until the
  mouse is released.
- **What "the same thing" means** when two peers edit one object. A CRDT merges
  at the granularity you give it, and that granularity decides which concurrent
  edits survive. This is the one of the three that is a *file format* decision
  as well as a behaviour decision, so it is the most expensive to change.

ADR 0008 removes the fallback that would otherwise soften all three: the server
holds ciphertext, so there is no server-side repair, inspection or
reconciliation of a document that ends up in a state we did not intend.

### Options considered — undo scope

**A. Global undo: Ctrl+Z reverts the last change to the document, whoever made
it.** What a single-user editor does, and what a user coming from Inkscape
expects. Rejected: with concurrent authors it is both surprising and unsafe —
my Ctrl+Z silently discards work my collaborator is still looking at, and two
peers pressing Ctrl+Z at the same time race on the same entry. It also cannot
be built on a CRDT without a coordinator that serializes undo, which is the
authoritative server ADR 0004 rejected.

**B. Peer-scoped undo: each peer undoes only its own commits, remapped against
intervening remote edits.** What Google Docs, Figma and Zed do. Chosen. The
cost is honest and narrow: "undo the last change to this document" is not a
feature we have, and a user who watches a collaborator make a mistake cannot
press Ctrl+Z to fix it.

**C. Peer-scoped undo plus an explicit "revert this change" action in a history
view**, able to target any past commit including another peer's. This is B plus
a feature, and it is where we should end up — but as a deliberate, labelled,
non-destructive-looking action, never bound to Ctrl+Z. Deferred to a story
rather than rejected (§1).

**D. Locking: only one peer may edit an object, so undo is unambiguous.**
Rejected as ADR 0004 option C, for its own reasons; it is noted here because it
is the only option that makes global undo safe, and the price is exactly the
collaboration the customer asked for.

### Options considered — merge granularity

**A. The document is one value.** Simplest, and wrong: two peers editing
different objects would conflict, which is the whole problem.

**B. Per node.** Each node a value, replaced wholesale on edit. Rejected: two
peers editing different anchors of the same path is a case the customer will
hit immediately (one adjusts a curve while the other moves its end), and this
loses one of them. ADR 0004 rejected per-object locking on the stated promise
that editing the same path merges rather than being forbidden; per-node
granularity would quietly withdraw that promise.

**C. Per field, with sequences where order is semantic** (§3). Chosen. Costs
more operation metadata per edit and more care in the model, and still has a
last-writer-wins floor at the scalar level — which is stated plainly rather
than papered over.

**D. Per field, with numeric merging of concurrent scalar edits** (e.g.
averaging two positions, or summing two deltas). Rejected: for a coordinate
this invents a third value neither peer asked for, and for manufacturing
parameters — power, speed, depth — a merged value nobody chose is a ruined
workpiece or a fire risk. Last-writer-wins is worse than nothing in theory and
much better than invention in practice.

## Decision

1. **Undo and redo are peer-scoped, and the UI says so rather than implying a
   global stack.**
   - A peer's stack holds only commits that peer originated. Ctrl+Z reverts
     *my* last change, never a collaborator's, and their changes neither clear
     nor reorder my stack. Redo is symmetric: a remote change does not
     invalidate it, a new local command does.
   - Undo is a **new forward commit** that reverts the effect, remapped against
     intervening remote edits — not a rewind of history. The log only grows, my
     undo replicates like any other edit, and it cannot clobber a concurrent
     change the way a pre-computed inverse would (ADR 0002, rejected option 2).
   - Where remapping has no meaning — a peer deleted the node I created, or the
     group I moved it into — that entry is skipped with a message. Undo never
     resurrects content a peer removed.
   - **With nobody else connected this is indistinguishable from classic linear
     undo**, which is the customer's normal case and what makes the decision
     affordable.
   - The stack is session-scoped and bounded by a configurable step count,
     cleared on close even though the log behind it persists. The persisted log
     is what a *history view* reads, so "see what changed" can survive a reopen
     while "undo it" does not.
   - Option C — reverting an arbitrary past commit, including a collaborator's
     — is a later story and an explicit history-view action, never Ctrl+Z.
2. **Ephemeral state is not document state.** Pointer position, selection,
   hover, active tool and the in-flight geometry of a drag or live transform
   travel over a separate awareness channel: never operations, never in the
   log, never in a saved file, never undoable. This is what makes ADR 0002 §9's
   one-commit-per-interaction affordable — collaborators see the drag live as
   presence, and the document gains exactly one entry when the mouse is
   released. Selection being local also means a `NodeId` held in local state
   can dangle when a peer deletes the node, so local state resolves ids lazily
   and drops the missing ones; a case that could not arise with a single
   writer.
3. **Merge granularity is chosen per field and is part of the document format.**
   - A path's anchors are a **movable list**, each anchor a map of fields, so
     two peers editing different anchors of one path both keep their work.
   - A text node's string is a **text container**: concurrent typing merges per
     character instead of one side overwriting the other. This is the one place
     a vector editor has the same problem as a text editor, and it costs
     nothing to use the container that solves it.
   - Scalars — transform components, stroke width, colours, radii, job
     parameters — are **per-field last-writer-wins registers**. Field
     granularity makes "one peer changes the fill, another the stroke width"
     conflict-free, and also means two peers dragging the *same* node produce
     one of the two positions. There is no numeric merge (option D).
     **"A CRDT does not lose work" is false at this granularity** and no UI
     copy may suggest otherwise.
   - The SVG passthrough bag (ADR 0002 §10) and embedded job snapshots
     (ADR 0002 §8) are opaque values, replaced wholesale, never merged.
4. **Every read that produces output runs against an immutable version
   snapshot** taken at invocation: SVG and machine-format export, toolpath and
   job generation, thumbnails, golden-file tests. A replica changes under you
   as remote updates arrive, and a job generated from a sliding state is
   neither reproducible nor necessarily self-consistent — for a machine that
   cuts material, that is the difference between a defect and a scrapped
   workpiece. With ADR 0002 §5's CRDT-derived ordering this makes output a
   deterministic function of (version, parameters), so two peers exporting one
   version produce identical bytes.
5. **A missing font is a visible condition, and a substituting peer never
   writes layout back.** Fonts are referenced, not embedded by default
   (ADR 0004 §12), so a collaborator may not hold the face. That peer renders a
   substitute, marked as substituted in the UI, and is **blocked at the API
   level from writing re-measured layout or converting that text to paths** —
   otherwise one peer's missing font silently rewrites the document for the
   peer who has it. Embedding fonts when sharing is an explicit action, as on
   export.

## Consequences

- **"Undo the last change to this document" is not a feature we have**, and the
  gap is visible the first time two people work in one file. §1's history-view
  revert is the answer and it is a story, not shipped behaviour. Recorded in
  `docs/technical-debt.md`.
- **A contested scalar loses one side's value** (§3). Nothing is corrupted and
  nothing is irrecoverable — the log holds both — but one peer's edit visibly
  does not take effect, and the only honest mitigation is presence: the user
  must be able to see that someone else is holding the object before grabbing
  it. That makes §2's awareness channel a correctness-adjacent feature, not a
  nicety. Recorded in `docs/technical-debt.md`.
- Undo growing the log rather than shrinking it (§1) means an edit-and-undo
  session costs twice its apparent size, which feeds the compaction entry in
  `docs/technical-debt.md`.
- §4 widens an API: exporters and toolpath generators take a version handle
  rather than "the document". Cheap now, and the thing that makes job output
  reproducible under collaboration.
- §2 is a second channel to build, test and seal (ADR 0008 applies to presence
  as much as to edits — a cursor position is user data). It is small, but it is
  not free, and it must not become a back door that carries document content.
- Per-field granularity (§3) puts operation metadata on every scalar. Combined
  with per-node resolved styles (ADR 0002 §5), a large restyle or transform of
  many objects is a measurable amount of log. Compaction is the mitigation and
  already has an entry.
