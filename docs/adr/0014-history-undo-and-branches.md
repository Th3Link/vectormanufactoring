# ADR 0014: History, undo and branches over the operation log

**Status:** Proposed (2026-10-10, amended 2026-10-10 after the Loro spike), `needs-customer`
(document model, persistence and sync, `CLAUDE.md` §3). Seven questions at the end; each has a
default. On acceptance, one sentence of [ADR 0004](0004-persistence-and-cross-machine-sync.md) §2
("Loro's peer-scoped undo manager is the mechanism") is superseded by §1 below, and 0004's Status
line gets a pointer here.

## Context

`specs/0020-undo-redo`, `0041-object-history` and `0042-history-branches` ask for per-user Ctrl+Z,
a document history with step ids, authors, times and touched objects, per-object undo (Ctrl+U)
that is itself a step, clones that keep their history, wipes, branches, previews and clones of any
past state. The accepted base stays: the Loro operation log is the history, undo is a new forward
commit, peer-scoped, never resurrects what a peer deleted (ADR 0002 §9, ADR 0009 §1).

Facts this ADR builds on. From the code on `main` (`798a8ad`): every editing method ends with
`commit_with_label(label)` (persisted commit message, no timestamp); `document.loro` is
`ExportMode::Snapshot`, so **every saved file already holds the full log**, deleted objects
included; `NodeId` is the Loro `TreeID`; anchor ids are stored values, not container ids.
From the spike [`docs/spikes/loro-history-primitives.md`](../spikes/loro-history-primitives.md)
(Loro 1.16.2, 5,000 objects, 25,000 steps, native release; wasm not measured):

- Commit messages survive save and load unchanged. Commits with an identical message merge into one
  Loro change; a large commit splits into changes of about 1,300 ops. So one commit is not one
  change, and grouping by header (§2) is required.
- Loro forces change timestamps ascending, also across imported changes: one peer with a fast clock
  shifts everyone's later times.
- `checkout` costs grow with the ops between the two versions: 75 ms for 1,000 steps back, about
  1.4 s for 20,000. The first checkout after Open builds a history cache: 261 ms and +48 MB at
  154,000 ops. Every `Document` writer panics while the document is detached.
- `diff(a, b)` returns the values of exactly the keys that changed, without detaching: 0 ms for the
  newest step, 9 ms at 100 steps back, 102 ms at 1,000.
- `fork()` and `fork_at()` return **editable** documents with a new peer: 113 to 274 ms and 0.6 to
  3 s, plus a cold first checkout and read on the fork. A preview from a fork misses 0042's 300 ms.
- **A deleted tree node cannot be restored with its id through the handler API**: `mov` on it
  errors; `apply_diff`, `revert_to` and `UndoManager` re-create it under a new `TreeID`; a
  re-inserted container is a new, empty one. See the options below.
- `get_changed_containers_in` reports only the root tree for a create or delete, and
  `get_path_to_container` fails for nested containers of deleted nodes (645 of 8,120). The ops in
  `export_json_updates_without_peer_compression` resolve every case.
- A shallow snapshot keeps every `NodeId`, tombstone, root register and the peer counter (31 % of
  the full snapshot). A replica holding pre-trim history can no longer merge, and Loro does not
  report that reliably (everything `pending`, or a silently incomplete update).
- `Document::new` leaves its 3 root ops uncommitted; the first user commit carries them (12 ops).

### Options: the undo engine

**A. Loro's `UndoManager`.** Free remapping against remote edits. Rejected as the engine: it
applies a transformed inverse diff with no veto per object (exclusion is by origin string only), so
0020 criteria 17 to 19 are not expressible; undoing a delete re-creates the node under a **new**
`TreeID`, which breaks ADR 0002 §5; and 0041/0042 need a second engine anyway ("set this object to
its state at version k").

**B. Our own restore engine over versions** (chosen). A step has a version before and after. Undo
reads the values the step changed at both versions and writes, field by field, the old value
wherever the current value is still the one the step wrote. Object undo, go to version and clone
version are the same read with a different write rule. One engine, rules we own and test, no stored
inverses.

**C. `revert_to(version)`.** Reverts every peer's later work and re-creates deleted nodes under new
ids. Rejected for undo (ADR 0009 option A); it is also why the Document scope gets no Live mode
(0042 Q3).

### Options: restoring a deleted object with its `NodeId`

**A. Injected tree `Move` op** (chosen, Q7 default). One JSON change of this peer through
`import_json_updates`: no format change, same `NodeId`, same anchor containers, old z-order,
0.22 ms in the spike. Risk:
it relies on CRDT behaviour the handler API forbids and the Loro docs do not promise. **B. Soft
delete:** Delete moves the node under a hidden "trash" tree node; undo moves it back with the public
API. Works in the spike, but older builds list the trash node as an object, so it needs a
`format_version` bump and a policy for trash growth. Kept as the fallback (Q7). **C. Re-create
under a new id plus an alias.** Rejected: breaks ADR 0002 §5 and every `NodeId`-keyed structure
(selection, `clone_of`, timelines).

### Options: where a step's boundary lives

**A. Loro change boundaries.** Rejected: splits and merges (above). **B. A step header in the
commit message** (chosen): persisted, replicated, free, already used for the label. **C. A marker
container written by every commit.** Rejected: extra ops per step, a new container in the model,
and a marker a peer could edit concurrently.

## Decision

1. **Engine: restore over versions (option B).** For each object a step touched: before `B`,
   after `A`, now `N`. Undo writes `B.f` for each field with `N.f == A.f` (exact stored-value
   equality, not a geometric tolerance: the values were written, not measured). Anchors match by
   anchor id; the anchor order, the z-position and the tree parent are fields. Created by the step:
   removed only if `N == A`, else kept and reported. Deleted by the step: restored **with the same
   `NodeId`** (§11) only if its last tree op is still the step's delete. Gone now: skipped. Redo
   swaps `B` and `A`. The result is one ordinary commit, replicated like any edit.
   *Amended 2026-10-10:* `A` and `B` come from `diff` between the step's before and after versions,
   restricted to the step's touched containers; `N` is a live read. **Undo does not check out.**
   A `checkout` is used only to read whole objects at a version (preview, go to version, clone
   version, inherited rows), always through a guard (§12).
2. **Step header.** A step is all changes of one peer whose commit message carries the same header
   `<label>;s=<seq>;t=<seconds>[;<key>=<value>]…`: `s` is the session's step counter, `t` the
   step's wall-clock time (§4); optional keys name the step a revert targets (`u` undo, `d` redo,
   `ou`/`od` object undo/redo, with `o` the object), a go-to or clone version (`v`), and are
   ignored when unknown. A **continuation** repeats the header of the peer's last step verbatim and
   joins it (its time is the step's start); it is allowed only while the document version is still
   that step's last version (0044 held nudge, 0042 exploring go-to). A message without `;` is a
   legacy label: consecutive same-peer changes with that label are one step. Identical messages
   merging into one Loro change and large commits splitting are both harmless under this rule.
3. **Step identity.** `StepRef` = the id (peer, counter) of the step's first op, the same on every
   replica. Shown id: FNV-1a 64 of the 12 bytes, base36 lowercase, 7 characters, lengthened on a
   collision within the document. Hand-written, no dependency.
4. **Time and author.** *Amended 2026-10-10:* the editor passes wall-clock seconds in
   (`Document::set_step_time`); `commit_with_label` writes them as `t=` in the header. Loro change
   timestamps are not set and not read: Loro clamps them ascending across peers, so they cannot
   carry each peer's own time. A step without `t` is "Unknown time". The core reads no clock.
   Author = the peer id, a random number per session: "You" for the current peer, "Earlier
   session" otherwise. No names in the file (Q5).
5. **Touched objects and kind are derived from the step's ops, never stored.** *Amended
   2026-10-10:* the source is `export_json_updates_without_peer_compression` (or
   `export_json_in_id_span` for a few steps), not `get_changed_containers_in` or
   `get_path_to_container`. Tree `Create`/`Move`/`Delete` ops name their target; every other op's
   container maps to its object through a child-to-parent map built from the container-creating ops
   up to a node's meta map (`TreeID::associated_meta_container()`). Deleted objects resolve like
   live ones. Computed per visible row and cached by `StepRef` (a step's ops never change); the
   per-object index (0041) is one pass built on first request (about 90 ms warm, 220 ms cold at
   25,000 steps), then extended per commit or import from that change's ops only.
6. **One stack machine.** A pure transition function over do, undo, redo, continuation steps gives
   the stacks, each step's status (applied, undone, on a branch) and the branches (steps left on
   the redo side when a new do step arrives; fork point = the top of the undo side). The session
   feeds it its own actions (live stacks, 500 entries); the history list feeds it the log, per peer
   and per object. Main-line order of concurrent steps: (Lamport, peer). Selection per entry stays in
   memory (view state, ADR 0009 §2).
7. **Persistence.** History is the log `document.loro` already holds: no new member, no
   `format_version` bump for 0020 (with Q7 A). 0041 adds two optional keys on an object,
   `history_floor` (the version at an object wipe) and `clone_of` (`<NodeId>@<version>`), and
   0020's wipe writes one root key `history_wiped`; older builds ignore all three (Q2).
8. **Previews.** *Amended 2026-10-10, replaces "previews use a forked copy":* no fork. A preview
   reads the objects it shows from the live document inside one guarded `checkout` (§12), copies
   them into plain `ObjectSnapshot`s (a `HistoricalDocument` that is data, not a `LoroDoc`, so it
   cannot be edited), and returns to latest in the same call. The live document keeps editing and
   merging between rows. Measured: 125 to 250 ms for a row 1,000 steps back with all 5,000
   objects; far rows take longer (budgets, §13).
9. **Document wipe** = `export(shallow_snapshot(current frontiers))`, re-imported in place under the
   same peer; object ids, tombstones and state unchanged, older ops gone, stacks emptied. Offered
   only for a document without a keyring (Q3). *Amended 2026-10-10:* the wipe step writes the root
   key `history_wiped` = the **epoch**, the encoded shallow-start frontiers (0041 decision 9
   encoding), the same on every replica that has the wipe. Because Loro does not reliably report a
   failed merge across a trim, every exchange of updates (the `import_updates` seam of 0020 now,
   the sync layer later) compares epochs first and refuses a mismatch; `import_updates` also
   treats a non-empty `pending` as an error.
10. **Object wipe hides and cuts**: the `history_floor` register; ops stay (Q4). Removing one
    object's ops from a replicated log is not possible without rebuilding the document for every
    peer.
11. **Reviving a deleted node** (*new 2026-10-10*, option A, Q7). Undo of Delete, redo of Create,
    object undo of a creation's removal and Restore of a deleted object all use one function in
    `history/revive.rs`: commit everything pending, then import one JSON change of this peer with
    the next counter, `deps` = oplog frontiers, Lamport = the highest `lamport + len` among the
    frontier changes, the step's header as message, and one tree `Move { target, parent: None,
    fractional_index }`. The index is the node's old one when its old neighbours are unchanged,
    else one between the current neighbours. The restore commit for the rest of the step follows
    with the same header, so both changes are one step. Guards: `loro` is pinned exactly
    (`=1.16.2`, like `i_overlay`), and a canary test in `curvyo-document-core` fails when Loro
    stops accepting the op or stops keeping the containers. A Loro upgrade that breaks the canary
    is not taken; soft delete (option B) is then built behind a customer decision on the bump.
12. **Checkout guard** (*new 2026-10-10*). The only way to check out is a guard value whose `Drop`
    calls `checkout_to_latest`; it exposes reads only, and `Document::version()` and every cache
    keyed by `DocumentVersion` are unreachable while it lives. Detached editing
    (`set_detached_editing`) is never enabled: it changes the peer id for good.
13. **Performance budgets** (*new 2026-10-10*, release build, desktop; the tests assert four times
    each figure; wasm is measured in milestone 1 and recorded as a factor, not a separate budget):
    - **Undo and redo, small steps** (at most 2,000 ops, read from the step's change lengths before
      the call): 100 ms, with nothing shown. This covers every single-object edit and moves of a
      few hundred simple objects.
    - **Large steps** (more than 2,000 ops): the editor enters the long-operation state of
      `specs/0016-boolean-operations` §9 **before** the call (cursor `wait`, `aria-busy`, other keys
      ignored, "Undo: working..." after 150 ms). Budgets: 100 paths x 100 anchors 250 ms, 5,000
      objects 1 s. The spike's read cost alone was 129 ms and 340 ms with checkouts; `diff` is
      expected to be lower, the writes come on top.
    - **First history call after Open**: the editor calls `Document::warm_history()` in an idle
      callback after the first paint (one `diff` of the newest step). Until it has run, an undo
      uses the long-operation state. The checkout cache (+48 MB, about 260 ms at 154,000 ops) is
      built only by the first checkout-based read, behind the same state, and freed with
      `free_history_cache` when the History tab closes. If milestone 1 finds that `diff` after
      Open needs the same cache, `warm_history` builds it and the memory is paid by every open
      document with history; the budget then states it.
    - **Preview** (0042 criterion 21): first frame within 300 ms for rows up to 1,000 steps back
      from the current version; further back the row shows the long-operation state and the
      frame within 1.5 s at 10,000 steps back.
    - **Object timeline** (0041 criterion 35): 100 ms once the index exists; the first build
      (up to 300 ms cold at 25,000 steps) runs behind the long-operation state.

## Consequences

- ADR 0009 §1 holds as written; ADR 0004 §2's mechanism sentence is replaced. The remapping that
  Loro would have done for lists is ours: anchors by id, which our ids make exact.
- Undo of Delete depends on a pinned Loro version and an op the public API forbids. A Loro upgrade
  costs a canary run; if it fails, we stay on the pinned version until soft delete (Q7 B) and its
  format bump are approved and built.
- Undo reads by `diff`, so the live document is never detached for an undo and a panic cannot leave
  it detached. Reads of whole past states detach it briefly under the guard.
- Large undos are visible: a busy cursor and, past 150 ms, a working notice. Small ones stay at
  100 ms.
- The first checkout-based history read after Open costs memory in proportion to the log (+48 MB at
  154,000 ops) until the History tab closes.
- Undo stacks hold refs and selections, not inverses: 500 steps are kilobytes, not 50 MB.
- An undo that skips everything writes nothing, so the log does not show it; the live stack has
  consumed it, the list shows the step as applied (its effect was overwritten by others).
- Files written before this ADR show merged rows where two same-label commits were adjacent, and
  "Unknown time". Their first row also contains the document's root initialisation.
- A wiped document can no longer merge with a copy that holds pre-wipe edits it never saw; the
  epoch check turns that into a clear refusal instead of a silent partial merge.
- An older build duplicating an object copies `clone_of` verbatim: a wrong lineage label in a newer
  build, never a wrong drawing.

## Verification

The spike (`spike/loro-history-primitives`, never merged; results in
[`docs/spikes/loro-history-primitives.md`](../spikes/loro-history-primitives.md)) answered S1 to
S7: S1 fails through the handler API and works through the injected move (§11); S2, S3, S5 and S7
pass; S4 works with the costs above; S6 holds for meta maps (`associated_meta_container`) and,
for nested containers of deleted nodes, through the op parent chain only (§5).

**Milestone 1 of `0020` verifies again, as tests on the branch** (milestone 0 is done):

1. Canary: the injected move revives the same `NodeId`, anchor containers and z-order, survives
   save and load, and a peer's edit made while the node was deleted survives (not tested for the
   injected move in the spike).
2. The injected change's Lamport and deps after importing another peer's newer changes; a
   concurrent delete by a peer; the fractional index between changed neighbours
   (`new_between_jitter`, not tried).
3. `import_json_updates` and the JSON schema types build with `default-features = false` and for
   `wasm32-unknown-unknown`.
4. `diff` between two historical versions (a step 1,000 and 10,000 steps back): cost and that it
   yields `A` and `B` for every touched field, anchor order and tree parent included.
5. §13 budgets with the restore writes included, warm and right after Open, plus one wasm figure.
6. The checkout guard returns to latest when a read panics.
7. `t=` is shown for each peer as written after importing a change with a later Loro timestamp.
8. Touched objects from the JSON ops for deleted nodes' nested containers; index cost.
9. The commit-path audit once `0019` has merged (the spike found `transform_objects` to be one
   commit there).
10. The wipe: epoch written, a pre-wipe replica's updates refused, `pending` treated as an error.

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
7. **Undo of Delete** (new 2026-10-10). *A (default, recommended):* revive the object with an
   injected Loro op (§11); no file-format change; Loro stays pinned and a canary test guards every
   upgrade; soft delete stays the fallback and comes back to you only if a Loro upgrade we need
   breaks the canary. *B:* soft delete now: deleted objects move into a hidden trash inside the
   file; public Loro API only, but a `format_version` bump (older builds refuse new files) and
   deleted objects stay in the file until a wipe. *C:* undo of Delete brings the object back under
   a new id: no risk, but selections, clone lineage and the object's timeline break at every
   undone delete. Not recommended.
