# ADR 0014: History, undo and branches over the operation log

**Status:** Proposed (2026-10-10), `needs-customer` (document model, persistence and sync,
`CLAUDE.md` §3). Six questions at the end; each has a default. On acceptance, one sentence of
[ADR 0004](0004-persistence-and-cross-machine-sync.md) §2 ("Loro's peer-scoped undo manager is the
mechanism") is superseded by §1 below, and 0004's Status line gets a pointer here.

## Context

`specs/0020-undo-redo`, `0041-object-history` and `0042-history-branches` ask for per-user Ctrl+Z,
a document history with step ids, authors, times and touched objects, per-object undo (Ctrl+U)
that is itself a step, clones that keep their history, wipes, branches, previews and clones of any
past state. The accepted base stays: the Loro operation log is the history, undo is a new forward
commit, peer-scoped, never resurrects what a peer deleted (ADR 0002 §9, ADR 0009 §1).

Facts this ADR builds on. From the code on `main` (`52d4101`): every editing method ends with
`commit_with_label(label)` (persisted commit message, no timestamp); `document.loro` is
`ExportMode::Snapshot`, so **every saved file already holds the full log**, deleted objects
included; `NodeId` is the Loro `TreeID`; anchor ids are stored values, not container ids;
`Document::new` leaves its root registers in an uncommitted transaction that the first labelled
commit swallows. From the Loro 1.16.2 API docs (not the source, which is outside the tree; every
name is **to be verified at build**, see "Verification"): `UndoManager` (local peer only, merge
interval, max steps, `on_push`/`on_pop` metadata, no per-item veto); `ChangeMeta { id, lamport,
deps, timestamp (s), message, len }`; `CommitOptions::timestamp`; `checkout`, `fork`, `fork_at`,
`diff`, `apply_diff`, `revert_to`, `travel_change_ancestors`; `ExportMode::shallow_snapshot`.
One commit is not one Loro change: a large commit splits into several changes, and consecutive
changes with one message can merge (`docs/technical-debt.md`).

### Options: the undo engine

**A. Loro's `UndoManager`.** Free remapping against remote edits. Rejected as the engine: it
applies a transformed inverse diff with no veto per object, so 0020 criteria 17 to 19 (skip an
object a peer deleted, keep an object a peer changed after I created it, one press consumes exactly
one entry) are not expressible; its behaviour on tree deletes and skipped items is undocumented;
and 0041/0042 need a second engine anyway ("set this object to its state at version k").

**B. Our own restore engine over versions** (chosen). A step has a version before and after. Undo
reads the touched objects at both versions and writes, field by field, the old value wherever the
current value is still the one the step wrote. Object undo, go to version and clone version are the
same read with a different write rule. One engine, rules we own and test, no stored inverses.

**C. `revert_to(version)`.** Reverts every peer's later work. Rejected for undo (ADR 0009 option A);
it is also why the Document scope gets no Live mode (0042 Q3).

### Options: where a step's boundary lives

**A. Loro change boundaries.** Rejected: splits and merges (above). **B. A step header in the
commit message** (chosen): persisted, replicated, free, already used for the label. **C. A marker
container written by every commit.** Rejected: extra ops per step, a new container in the model,
and a marker a peer could edit concurrently.

## Decision

1. **Engine: restore over versions (option B).** For each object a step touched: before `B`,
   after `A`, now `N`. Undo writes `B.f` for each field with `N.f == A.f` (exact stored-value
   equality, not a geometric tolerance: the values were written, not measured). Anchors match by
   anchor id; the anchor order and the z-position are fields. Created by the step: removed only if
   `N == A`, else kept and reported. Deleted by the step: restored **with the same `NodeId`**.
   Gone now: skipped. Redo swaps `B` and `A`. The result is one ordinary commit, replicated like
   any edit. States at a version are read by `checkout` on the live document inside one synchronous
   call and `checkout_to_latest` before returning; previews use a forked copy (§8).
2. **Step header.** A step is all changes of one peer whose commit message carries the same header
   `<label>;s=<seq>[;<key>=<value>]…`: `s` is the session's step counter; optional keys name the
   step a revert targets (`u` undo, `d` redo, `ou`/`od` object undo/redo, with `o` the object), a
   go-to or clone version (`v`), and are ignored when unknown. A **continuation** repeats the header
   of the peer's last step and joins it; it is allowed only while the document version is still
   that step's last version (0044 held nudge, 0042 exploring go-to). A message without `;` is a
   legacy label: consecutive same-peer changes with that label are one step.
3. **Step identity.** `StepRef` = the id (peer, counter) of the step's first op, the same on every
   replica. Shown id: FNV-1a 64 of the 12 bytes, base36 lowercase, 7 characters, lengthened on a
   collision within the document. Hand-written, no dependency.
4. **Time and author.** The editor passes wall-clock seconds in (`Document::set_step_time`); the
   commit carries it via `CommitOptions::timestamp`; the core reads no clock. A change with
   timestamp 0 is "Unknown time". Author = the peer id, a random number per session: "You" for the
   current peer, "Earlier session" otherwise. No names in the file (Q5).
5. **Touched objects and kind are derived from the step's ops, never stored**: tree creates,
   deletes and moves by `TreeID`; changed containers mapped to their object (a node's meta map
   shares the node's id; nested containers through their path). Computed per visible row and
   cached by `StepRef` (a step's ops never change); the per-object index (0041) is one pass built
   on first request, then extended per commit or import.
6. **One stack machine.** A pure transition function over do, undo, redo, continuation steps gives
   the stacks, each step's status (applied, undone, on a branch) and the branches (steps left on
   the redo side when a new do step arrives; fork point = the top of the undo side). The session
   feeds it its own actions (live stacks, 500 entries); the history list feeds it the log, per peer
   and per object. Main-line order of concurrent steps: (Lamport, peer). Selection per entry stays in
   memory (view state, ADR 0009 §2).
7. **Persistence.** History is the log `document.loro` already holds: no new member, no
   `format_version` bump for 0020. 0041 adds two optional keys on an object, `history_floor`
   (the version at an object wipe) and `clone_of` (`<NodeId>@<version>`), and 0020's wipe writes
   one root key `history_wiped`; older builds ignore all three (Q2).
8. **Previews** run on `fork()` of the live document, checked out per row and dropped when the
   preview ends. The live document keeps editing and merging.
9. **Document wipe** = `export(shallow_snapshot(current frontiers))`, re-imported in place under the
   same peer; object ids and state unchanged, older ops gone, stacks emptied. Offered only for a
   document without a keyring (Q3).
10. **Object wipe hides and cuts**: the `history_floor` register; ops stay (Q4). Removing one
    object's ops from a replicated log is not possible without rebuilding the document for every
    peer.

## Consequences

- ADR 0009 §1 holds as written; ADR 0004 §2's mechanism sentence is replaced. The remapping that
  Loro would have done for lists is ours: anchors by id, which our ids make exact.
- An undo costs two checkouts of the live document. Measured against 0020 criterion 30 in
  milestone 1; fallback is a shadow fork kept in sync (twice the document memory).
- Undo stacks hold refs and selections, not inverses: 500 steps are kilobytes, not 50 MB.
- An undo that skips everything writes nothing, so the log does not show it; the live stack has
  consumed it, the list shows the step as applied (its effect was overwritten by others).
- Files written before this ADR show merged rows where two same-label commits were adjacent, and
  "Unknown time". Their first row also contains the document's root initialisation.
- A wiped document can no longer merge with a copy that holds pre-wipe edits it never saw.
- An older build duplicating an object copies `clone_of` verbatim: a wrong lineage label in a newer
  build, never a wrong drawing.

## Verification (spike `spike/loro-history-primitives`, never merged; results recorded here)

S1 restore a deleted tree node with its old `TreeID` (`apply_diff` tree create, or move back);
S2 two commits with different messages never share a change; S3 `CommitOptions::timestamp` survives
Snapshot export without `set_record_timestamp`; S4 checkout and back at 5,000 objects and 10,000
steps; S5 shallow snapshot keeps `TreeID`s and the peer's counter continues; S6 meta-map
`ContainerID` = node id, nested containers resolvable for deleted nodes; S7 ops of one step
readable (`diff` or `export_json_updates`). If S1 fails, undo of Delete needs a soft-delete parent
and a format bump: a new question to the customer before milestone 1 merges.

## Questions for the customer

1. **History in the file** (0020 Q1). It is already there today; this makes it visible, with
   times. *A (default, recommended):* keep it, the History tab says so, wipe available.
   *B:* strip history on every Save.
2. **File-format footprint.** *A (default, recommended):* no `format_version` bump; step headers in
   commit messages and three optional keys that older builds ignore. *B:* bump at 0041 so older
   builds refuse files with lineage keys.
3. **Wipe in a shared document.** *A (default, recommended):* not offered until sharing exists.
   *B:* admins only, with a new key epoch and fresh sealed snapshot (ADR 0010 §8); copies other
   people already hold keep the old history. *C:* no wipe at all.
4. **Object wipe** (0041 Q3). *A (default, recommended):* hides and cuts, data stays, the
   confirmation says so. *B:* no object wipe.
5. **Authors.** *A (default, recommended):* random per-session id, "You" and "Earlier session".
   *B:* one random id per installation in every step, so "You" survives reopening, but every file
   you share carries the same identifier. *C:* the OS user name in the file.
6. **Taking back someone else's step** with Ctrl+U or go to version (0041 Q1, 0042 c15; ADR 0009
   option C, never Ctrl+Z). *A (default, recommended):* allowed, as a named step that Ctrl+Z takes
   back. *B:* own steps only.
