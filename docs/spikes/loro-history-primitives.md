# Spike: Loro history primitives (ADR 0014)

**Date:** 2026-10-10. **Status:** results only; the throwaway code is not merged.
**Verifies:** the Loro API assumptions of [ADR 0014](../adr/0014-history-undo-and-branches.md)
(`Proposed`) and of `specs/0020-undo-redo`, `0041-object-history`, `0042-history-branches`
(`adrs.md` milestone 0).
**Loro:** 1.16.2 (`Cargo.lock`; workspace requirement `1.16`, `default-features = false`).
**Base:** `main` at `798a8ad`.

## Verdict first

| ADR 0014 check | Result |
|---|---|
| S1 restore a deleted node with its old `TreeID` | **Fails through the public handler API**, works through an injected tree `Move` op. See section 4. |
| S2 two commits with different messages never share a change | Passes. Commits with an identical message do merge. |
| S3 `CommitOptions::timestamp` survives `Snapshot` export | Passes. Timestamps are forced ascending, also by imported changes. |
| S4 checkout and back at 5,000 objects and 10,000+ steps | Works. Cost grows with distance in steps, not with object count. The first checkout after Open costs about 260 ms. |
| S5 shallow snapshot keeps `TreeID`s, peer counter continues | Passes. Replicas that hold older versions can no longer merge. |
| S6 meta-map `ContainerID` is the node id, nested containers resolvable for deleted nodes | Half. The meta map id is derivable. `get_path_to_container` fails for nested containers of deleted nodes. |
| S7 ops of one step readable | Passes through `export_json_updates_without_peer_compression`, `export_json_in_id_span` and `diff`. |

Two findings change the ADR: S1 (undo of Delete cannot use the documented API) and the checkout
and fork costs (section 3). Recommended amendments are at the end.

## Method

A throwaway test file `curvyo-document-core/tests/spike_loro_history.rs` drove the real
`Document` API (`create_rect`, `create_path`, `translate_objects`, `delete_objects`) on the spike
branch `spike/loro-history-primitives` (local only, not pushed). Two spike-only hooks were added
to `Document`: access to the inner `LoroDoc`, and a `commit_with_label` variant that writes
`<label>;s=<seq>` plus a timestamp, as ADR 0014 §2 and §4 describe.

- **Fixture:** 5,000 objects (70% rectangles, 30% closed paths with 4 anchors), then 20,000 steps
  (96% translate of 1 to 3 objects, 2% delete, 2% create). Result: 25,000 steps, 154,226 ops,
  25,000 changes, about 4,967 live and 399 deleted objects. Snapshot 2.47 MB.
- **Build:** release profile, `opt-level = 3`, no LTO, native x86-64 Linux. WebAssembly was **not**
  measured; expect it to be slower, by a factor not measured here.
- **Noise:** the machine ran other sessions (load average about 5 on 12 cores). Treat numbers as
  +/- 30%. Timed operations are medians of 5 unless marked "first".
- **Memory:** a counting global allocator. Heap figures around `fork()` are unreliable (`fork()`
  appears to compact the source's change store, so deltas go negative); the memory numbers below
  come from independent snapshot imports and from the cold-open sequence.
- **Cold** means a document opened from `document.loro` bytes (as after Open). **Warm** means built
  in the process.

## 1. Change metadata

`ChangeMeta { lamport: u32, id: ID, timestamp: i64, message: Option<Arc<str>>, deps: Frontiers, len: usize }`.
Reached with `LoroDoc::travel_change_ancestors(&[ID], &mut dyn FnMut(ChangeMeta) -> ControlFlow<()>)`
(visits newest first) or `LoroDoc::get_change(ID)`. The peer is `ChangeMeta.id.peer`.

- Messages written by `commit_with(CommitOptions::new().commit_msg(..).timestamp(..))` come back
  unchanged after `export(ExportMode::Snapshot)` and `from_snapshot`, and after
  `pack`/`unpack` of the `.curvyo` container (compared id, lamport, timestamp, len, message of
  every change). Tested: `move;s=12;u=ab12cd3;o=3@7`, `Résumé 日本 😀;s=1;k=v=w;;`, 2,000
  characters. An **empty** message reads back as `None`.
- `set_record_timestamp` is not needed when the timestamp is passed per commit. Without a
  timestamp the change has `timestamp == 0` (the "Unknown time" of ADR 0014 §4 works as written).
- **Option precedence:** `set_next_commit_timestamp(ts)` followed by
  `commit_with(CommitOptions::new().commit_msg(..))` keeps both: the message from `commit_with`, the
  timestamp from the pending next-commit options. `set_next_commit_message` is used when
  `commit_with` has no message. Explicit message in `commit_with` wins over
  `set_next_commit_message`. So `Document::set_step_time` can call `set_next_commit_timestamp`
  and the existing `commit_with_label` keeps working.
- **Timestamps are forced ascending** and the clamp includes imported changes. Local commit with
  `ts = 4000` after `ts = 5000` is stored as `5000`. A peer that imports a change stamped `5000`
  and commits with `ts = 1000` gets `5000`. A peer with a clock running ahead raises the displayed
  time of everyone's later steps.
- **Merging (S2):** 10 commits with the identical message `same` and no timestamp become **1
  change**. With distinct timestamps inside the default 1000 s merge interval they also become 1
  change. 10 commits whose messages differ (`same;s=0` .. `same;s=9`) stay 10 changes, as do 1,000
  alternating `A`/`B` commits. An empty `commit_with` adds no change. So steps must be grouped by
  header, as ADR 0014 §2 says; repeating a header (continuation) merges changes by itself.
- **Splitting:** a large commit is split into several changes of about 1,300 ops each: 1,300 map
  ops give 1 change, 2,000 give 2, 5,000 give 4, 20,000 give 15. `translate_objects` over 3,000 objects
  (one commit) gives 5 changes.
- **Peer compression pitfall:** `export_json_updates` (the default) writes small peer **indexes**
  (`Normal(Map 21@0)`) instead of peer ids. Use `export_json_updates_without_peer_compression`
  whenever ids are compared with `TreeID`s or `ChangeMeta.id`. `export_json_in_id_span` is not
  compressed.

## 2. Mapping changes to touched objects (ADR 0014 §5)

- The meta map of a node is `TreeID::associated_meta_container()`; equal to
  `tree.get_meta(id)?.id()` (checked). No state lookup is needed to go from `TreeID` to its meta
  container, also for deleted nodes.
- `get_changed_containers_in(ID, len)` gives container ids per change (66 ms for all 25,000
  changes warm, 132 ms cold) but **a tree create, move or delete only reports the root tree
  container**, not the target: 399 of 25,000 changes (exactly the deletes) had only root
  containers. It cannot name the object of a create or delete.
- `get_path_to_container(&ContainerID)` resolves nested containers of live objects
  (`root tree -> Node(TreeID) -> "anchors" -> Seq(i)`), 14 ms for 8,120 containers, but returns
  `None` for **645 of 8,120**: the anchors lists and anchor maps of deleted nodes. S6 fails for
  nested containers of deleted nodes through this call.
- What works for every case is one pass over the ops:
  `export_json_updates_without_peer_compression(&VersionVector::default(), &oplog_vv())`, then
  1. tree `Create`/`Move`/`Delete` ops name their `target` directly;
  2. a child-container-creating op is `Map(Insert { key, value: Container(child) })` or
     `MovableList(Insert { value: [Container(child)], .. })` on the parent container, so a
     `child -> parent` map is built from the ops themselves and resolved up to a meta map
     container whose `(peer, counter)` is a created `TreeID`.
  Deleted objects resolve like live ones.

| Index over 25,000 steps, 154,226 ops, 5,366 objects ever created | Warm | Cold |
|---|---|---|
| `travel_change_ancestors` over all changes | 12 ms | 126 ms |
| `export_json_updates_without_peer_compression` | 53 ms | 61 ms |
| parent map (8,120 entries) + attribution pass | 32 ms | not measured |
| **Per-object index (44,078 object-step entries, 0.6 MB)** | **about 90 ms** | about 220 ms (sum) |
| 50 newest rows via `export_json_in_id_span` + the prebuilt parent map | 0.1 ms | |

Criterion 35 of `0041` (no work on the commit path beyond appending) is feasible: extending the
index for one new change reads that change's ops only.

## 3. Checkout, detached mode, fork

### Checkout cost

Live document, warm, 5,000 objects, 25,000 steps:

| Steps back | `checkout` there | `checkout_to_latest` back |
|---|---|---|
| 1 | 0.02 ms | 0.02 ms |
| 10 | 2.3 ms | 2.1 ms |
| 100 | 3.9 ms | 3.6 ms |
| 1,000 | 75 ms | 18 ms |
| 5,000 | 322 ms | 52 ms |
| 20,000 | about 1,380 ms | 105 ms |

Cost follows the number of ops between the two versions, not the object count.

**Cold document (after Open):** opening costs 166 ms and 12 MB of heap (import is lazy; first
read of all 4,967 objects 125 ms, warm 44 ms). The **first checkout costs 261 ms** even for one
step, and heap goes from 12 MB to 61 MB (history cache of about 48 MB). Afterwards: 10 steps
11 ms, 100 steps 9 ms, 1,000 steps 199 ms, 5,000 steps 846 ms, 20,000 steps 2.6 s; back to
latest 0.1 to 177 ms. The cold document is 2 to 5 times slower than the warm one for 10 to 1,000 steps
until its caches are built. `free_history_cache` and `free_diff_calculator` drop the cache
again.

### Criterion 30 of `0020` (undo of 100 objects x 100 anchors within 100 ms, of 5,000 objects within 1 s)

The ADR's "two checkouts per undo" sequence (checkout before, read touched objects, checkout
after, read, `checkout_to_latest`), reads only, no writes:

| Step | Warm | Note |
|---|---|---|
| newest step, 3 objects | 0.09 ms | |
| 100 paths x 100 anchors (10,000 anchor writes) | 129 ms (checkouts alone 94 ms) | first run 198 ms; starting value 100 ms missed by 30%, the test limit (4x) holds |
| translate of 5,067 objects (370 changes) | 340 ms (checkouts alone 195 ms) | within the 1 s starting value |
| object undo of a step 1,000 steps back, 3 objects | 103 ms | distance dominates |

The restore engine's writes come on top. Not measured on wasm.

### Edits while detached

- Raw `LoroTree`/`LoroMap` writes while detached return `Err` ("Auto commit has not started. The
  doc is readonly when detached and detached editing is not enabled."). No op is written.
- `Document::translate_objects` **panics** (`unwrap` in `shape_codec.rs:130` on that error). Every
  `Document` writer does the same. A checkout must therefore always end with `checkout_to_latest`
  inside the same call, as ADR 0014 §1 already says; add a guard type so a panic cannot leave it
  detached.
- `set_detached_editing(true)` allows edits, but it **gives the document a new random peer id,
  and the id stays after `checkout_to_latest`**. Do not use it.
- `Document::version()` is `state_frontiers()`, so while detached it returns the checked-out
  version. Caches keyed by `DocumentVersion` must not be read during a checkout.

### Fork

`fork()` and `fork_at(&Frontiers)` return an ordinary, **editable** `LoroDoc` with a new random
peer id. A read-only preview needs a wrapper type; Loro offers none.

| 5,000 objects, 25,000 steps | Time |
|---|---|
| `fork()` from a cold document | 113 ms (warm live doc: 217 to 274 ms) |
| first read of all objects on the fork | 166 ms (live warm: 44 ms) |
| first `checkout` on that fork | 299 ms (history cache again), then 18 ms (100 back), 181 ms (1,000 back) |
| `fork_at` 1,000 steps back | 581 ms (warm source), 913 ms as the first history call on a cold document, 3.0 s after `free_history_cache` |
| `fork_at` 20,000 steps back | 2.0 s |
| independent document from the snapshot | 2.9 MB before reading, 12 MB after reading all objects |
| export `Snapshot` + import | 12 ms + 4 ms (lazy; the first real use pays) |

Criterion 21 of `0042` (first frame of a preview within 300 ms) is **not met** by `fork()` or
`fork_at()` at this size: fork plus first checkout plus first read-all is about 0.5 to 1.1 s.
Reading the same objects from the **live** document under a short checkout is cheaper: 1,000
steps back is 75 to 200 ms for the checkout plus 49 ms for reading all 4,976 objects, with no
second document in memory.

## 4. Restoring a deleted object with its old id (critical)

Objects are tree nodes. `NodeId` is the `TreeID`; every object's fields live in the node's meta
`LoroMap`; a path's anchors are a `LoroMovableList` of `LoroMap`s inside it. There is no
map key per object, so "re-insert under the same key" does not apply to the object itself. It was
tested on the meta map, too (e below).

What a deleted node still is: `tree.contains(id) == true`, `is_node_deleted(&id) == Ok(true)`,
`parent(id) == Some(Deleted)`, `get_meta(id)` still works, and the meta map and the `anchors`
container keep their container ids and content (the list is readable, length 4). It is hidden from
`roots()`.

| Attempt | Result |
|---|---|
| a. `tree.mov(id, Root)`, `tree.mov_to(id, Root, i)` | `Err`: "TreeID ... is deleted or does not exist". |
| c. `diff(after_delete, before_delete)` then `apply_diff` | `Ok`, but the diff is a tree `Create` plus map and list inserts: the object comes back under a **new `TreeID`** (29 ops, new container ids). Old id stays deleted. |
| d. `revert_to(before_delete)` | Same: new `TreeID`, 29 ops. It also reverts everyone's later work. |
| e. `meta.insert_container("anchors", new list)` on a **live** node | New container id. The key now points to the new, empty list; the old four anchors are gone from the state (the old container is still readable by id, `get_path_to_container` is `None`). |
| e0. any write to the meta map of a **deleted** node | `Err`: "The container cid:12@1:Map is deleted. You cannot apply the op on a deleted container." |
| g. peer B edits the object offline, A deletes, then merge | Import succeeds, the node stays deleted (delete wins), the tombstone keeps all four meta keys. |
| **h. tree `Move` op injected with `import_json_updates`** | **Works.** Same `TreeID`, same container ids, identical `object()` value, same z-index, editable with the normal API, survives snapshot export and import and checkout across it. |
| i. soft delete: `Delete` = `mov` under a hidden tree node, undo = `mov_to(id, Root, i)` | Works with the public API: same id, same containers, z-index restored, a peer's concurrent edit survives. Changes the format (below). |

Also for the record: Loro's own `UndoManager` undoing a delete re-creates the node under a **new**
`TreeID` (object `3@1` came back as `22@1`).

### The working revival (h), minimal form

Measured on the 5,000-object, 25,000-step document: **0.22 ms**, one change, one op.

```rust
// Everything pending must be committed first, so that oplog_vv() is complete.
doc.commit();
let vv = doc.oplog_vv();
let next = vv.get(&my_peer).copied().unwrap();            // next counter of this peer
let last = last_change_meta;                              // lamport + len of the newest change
let frac = /* FractionalIndex of the node's old Create op:
              doc.export_json_in_id_span(IdSpan::new(id.peer, id.counter, id.counter + 1)) */;
let mut schema = doc.export_json_updates_without_peer_compression(&vv, &vv); // empty change list
schema.changes.push(JsonChange {
    id: ID::new(my_peer, next),
    timestamp,                                            // step time
    deps: doc.oplog_frontiers().iter().collect(),
    lamport: last.lamport + last.len as u32,
    msg: Some("undo;s=12;u=<StepRef>".into()),
    ops: vec![JsonOp {
        content: JsonOpContent::Tree(JsonTreeOp::Move { target: id, parent: None, fractional_index: frac }),
        container: ContainerID::Root { name: "paths".into(), container_type: ContainerType::Tree },
        counter: next,
    }],
});
doc.import_json_updates(schema)?;
```

Observed after it: `ImportStatus.success = {my_peer: (next, next+1)}`, nothing pending; the next
normal edit gets counter `next + 1` (the local counter continues); the change keeps the message
and timestamp; a fresh replica built from the full snapshot has the object live;
`checkout(before)` shows it deleted, `checkout_to_latest` live again. The op is a normal CRDT
move: it wins against the delete by Lamport order, and it is concurrent with any other peer's
delete that the author had not seen (then Lamport and peer id decide, as for any concurrent
edit).

The risk: the public handler API forbids this move; the CRDT state accepts it. Nothing in the
Loro docs promises that behaviour. A Loro update that starts rejecting it would break undo of
Delete. The same node reused the old Create op's fractional index, which restores the old z-order
only if the siblings around it have not changed; a better index between the current neighbours
needs `FractionalIndex::new_between_jitter` (not tried).

### Children of a revived object

With (h) and (i) the node keeps its `anchors` list and every anchor map with the same container
ids, so nothing is copied. For (i) a peer's concurrent edit made while the node was in the trash
survived the restore (tested: first anchor x = 100); (h) restores the same containers, so the same
should hold, but it was not tested. With (c), (d) and any "re-insert" they are new containers with
copied values (and for (c)/(d) a new `NodeId`).

## 5. Object fields at a past version without a checkout

`LoroDoc::diff(&a, &b) -> LoroResult<DiffBatch>`; `DiffBatch::iter()` gives
`(&ContainerID, &Diff)`; `Diff::Map(MapDelta { updated: .. })` carries the value of **only the
keys that changed**, `Diff::Tree` the creates, moves and deletes, `Diff::List` inserts with child
container refs. The live document stays attached.

For a translate step of one rectangle and one 4-anchor path, `diff(after, before)` returned 5
entries: the rectangle meta map (`rect_bounds`) and the four anchor maps (`point`). That is
exactly what the restore engine needs (old value of each field the step wrote) plus the current
state for the three-way check, with no `checkout` at all.

| `diff(latest, n steps back)` | Time | Container entries |
|---|---|---|
| 1 | 0.0 ms (first call 48 ms) | 5 |
| 100 | 8.6 ms | 319 |
| 1,000 | 102 ms | 3,124 |
| 10,000 | 714 ms | 9,533 |
| 20,000 | 1,351 ms | 9,820 |

Other ways to read a version: `export(ExportMode::StateOnly(Some(frontiers)))` (337 ms, 818 KB,
import 17 ms, 1,000 steps back) and `ExportMode::SnapshotAt { version }` (275 ms, 2.40 MB, import
3 ms); both give a separate document. `fork_at` is 355 ms for comparison. Nested containers in a
`Diff` are identified by container id, which section 2's parent map maps back to the object and
the anchor.

## 6. Shallow snapshot (trim history)

Sizes, 25,000 steps: full `Snapshot` 2,474,367 bytes (`.curvyo` zip 1.71 MB);
`ShallowSnapshot(current frontiers)` 767,179 bytes (31%), 74 ms to export; keeping the last 1,000
steps 1,103,796 bytes (45%); `StateOnly(None)` 819,967 bytes (33%).

`LoroDoc::decode_import_blob_meta(bytes, false)` reports `mode = ShallowSnapshot` against
`Snapshot`.

After importing the shallow snapshot into a new document:

- All `object_ids()` identical and in the same order, every `object()` value identical.
- Deleted nodes are still tombstones (`contains == true`, `is_node_deleted == Ok(true)`).
- The root registers are present (`format_version` 9, size). `Document::from_loro_snapshot`
  accepts the bytes and `export_json` is byte-identical to before the trim. The manifest
  `format_version` and `loro_snapshot_version` are not involved; **no `format_version` bump** is
  needed for the trim itself. The file does carry a different Loro blob mode.
- `is_shallow() == true`, `shallow_since_frontiers()` is the one retained op; one change is
  visible, plus new ones.
- Under the same peer id the version vector already includes the old counters; the first new edit
  gets counter 154,226, equal to the old end counter, so the counter continues.
- Checkout inside the kept range works. Before the shallow start `checkout`, `diff` and
  `revert_to` return `Err("You cannot switch a document to a version before the shallow history's
  start version.")` and `fork_at` returns `Err("Cannot find (Frontiers(..))")`.

**Old replicas become unmergeable** (a copy that was forked before the trim and edited since):

| Case | Result |
|---|---|
| trimmed document imports the old replica's updates | `Err("Import Failed: The dependencies of the importing updates are not included in the shallow history of the doc.")` |
| old replica imports the trimmed snapshot | `Ok`, `success` empty, everything `pending`; nothing applies, no error |
| trimmed document exports updates since the old replica's version | `Ok`, 159 bytes: silently incomplete |
| a new replica imports the trimmed snapshot | works |

So a sync layer cannot rely on errors: after a wipe, a peer that still holds the old history has
to be refused by an explicit rule (a document epoch), not discovered by a failing import.

## 7. `UndoManager` (for the record, not for use)

`loro::UndoManager::new(&LoroDoc)`: per peer, the peer id is fixed at construction; `undo` and
`redo` take `&mut self` and return `Result<bool, LoroError>`. Other API: `group_start`,
`group_end`, `set_merge_interval(i64)` (seconds), `set_max_undo_steps(usize)`,
`add_exclude_origin_prefix(&str)` (excludes commits by **origin string**, not by object),
`record_new_checkpoint`, `can_undo`, `can_redo`, `undo_count`, `redo_count`, `top_undo_meta`,
`set_on_push`, `set_on_pop`, `pause`, `resume`, `clear`, `clear_undo`, `clear_redo`.

Tested: two `create_rect` commits without timestamps give `undo_count == 2` (default merge
interval does not merge them); undo of a delete restores the object under a **new `TreeID`**; undo
of the peer's own create after another peer deleted the node returns `Ok(false)` and consumes the
stack entry without error. There is no per-object veto. The documented `pause` keeps the stacks
across a temporary checkout.

## 8. `Document::new` and the commit paths

Confirmed on `main`:

- After `Document::new(7)`: `len_ops() == 3`, `len_changes() == 0`, `get_pending_txn_len() == 3`.
  The ops are `format_version`, `size_width_mm`, `size_height_mm`.
  `oplog_frontiers()` and `state_frontiers()` already show `2@7`, a position inside an
  uncommitted transaction. `version()` after `new` therefore names a version that no change
  contains yet.
- The first `create_rect` commit becomes one change of **12 ops** (3 init + 9 for the rectangle)
  with the message `create_rect`. Undoing that step would remove `format_version` and the size.
- `export_loro_snapshot` on a document that was never committed commits implicitly: one change of
  3 ops, **message `None`**, timestamp 0.
- A commit `new_document;s=0` right after `new` stays a separate change of 3 ops and does not
  merge with the following `create_rect;s=1` (different message). The milestone-1 plan of ADR
  0014 / `0020` decision 4 works.
- Multi-object commits: `translate_objects` over 100 objects is one commit and one change
  (202 ops, 0.56 ms). 100 per-object commits with the **same** header became 1 Loro change, so a
  per-object loop with one repeated header is one step, but it is still 100 commits and not
  atomic for a peer's import in between.
- **The `0019` resize is one commit, not one per object.** On
  `story/multi-object-transform` at `09c686b` (not on `main`), the group resize, rotate and skew
  go through `commit_group` to `Document::transform_objects`, which ends with a single
  `commit_with_label("transform_objects")` and writes nothing when nothing differs. The per-object
  writers (`resize_path`, `resize_rect`) are used only by single-object gestures. `main` has no
  multi-object resize yet. No `batch` is needed for it; the commit-path audit of `0020` task 1
  should recheck after `0019` merges.

## Recommended amendments to ADR 0014

1. **§1 and Verification S1: do not plan Delete-undo on `LoroTree::mov`.** Choose one, with a
   customer question if the format changes:
   - **A (recommended default):** revive with a tree `Move` op injected through
     `import_json_updates` (section 4, h). It keeps the `NodeId`, containers and z-order, costs
     0.22 ms, needs no format change. Guard it: pin `loro` with `=1.16.x` like `i_overlay`, add
     a test that fails when Loro stops accepting it, and keep B ready as the fallback.
   - **B:** soft delete: Delete moves the node under a hidden tree node ("trash"). Public API
     only, but it is a **format change** (older builds list the trash node as an object and call
     the file damaged), a bump, and trash growth needs a policy (wipe).
   - **C (reject):** re-create under a new `NodeId` plus an alias key. Breaks ADR 0002 §5 and
     every `NodeId`-keyed structure (selection, `clone_of`, object timelines).
2. **§1 read primitive: prefer `diff` over two checkouts.** `diff(after, before)` yields the old
   values of exactly the fields a step wrote (0.0 ms for the newest step, 9 ms at 100 steps, 102
   ms at 1,000) without detaching the live document. It also lowers the risk for criterion 30
   (checkout pair for the 100 x 100-anchor step: 129 ms warm). Keep checkout for reading whole
   objects at a version.
3. **§1 first undo after Open:** the first checkout builds the history cache (261 ms, +48 MB). Warm
   it after Open (one `checkout` of the previous frontiers and back, in an idle callback), or state
   the cost in criterion 30. The shadow-fork fallback does not help: a fork pays the same cold
   cost on its first checkout.
4. **§1 guard:** wrap checkout in a type that always ends with `checkout_to_latest`. Document
   writers panic while detached. `Document::version()` must not be read while detached.
5. **§2 and §4 time:** use `set_next_commit_timestamp` for `Document::set_step_time`; leave
   `commit_with_label` as it is. Because Loro forces timestamps ascending across imported
   changes, a skewed peer clock changes everyone's displayed times: put `t=<seconds>` into the
   step header as the authoritative step time and treat `ChangeMeta.timestamp` as a fallback for
   older steps. Costs about 12 bytes per step.
6. **§2 grouping:** keep grouping by header. Add to the text: commits with an identical message
   merge into one Loro change, and a commit splits into changes of about 1,300 ops
   (a 3,000-object commit gave 5 changes).
7. **§5 touched objects:** derive them from `export_json_updates_without_peer_compression` (tree
   op targets plus the child-to-parent map built from container-creating ops), not from
   `get_changed_containers_in` plus `get_path_to_container`: the first cannot see the object of a
   tree create/delete, the second fails for nested containers of deleted nodes. Per-object index
   build over 25,000 steps: about 90 ms warm, 220 ms cold.
8. **§8 previews:** do not fork. `fork()` and `fork_at()` miss the 300 ms first frame (0.5 to
   1.1 s), return an editable document, and cost 3 to 12 MB (+48 MB once checked out). Read the
   needed objects from the live document under one synchronous `checkout`, copy them into plain
   `ObjectSnapshot`s (a `HistoricalDocument` as data, 125 to 250 ms for 1,000 steps back), and
   `checkout_to_latest`. Rows far back (5,000 steps: 0.3 to 0.85 s; 20,000: 1.4 to 2.6 s) need a
   busy state; say so in `0042` criterion 21.
9. **§9 wipe and Consequences:** state that a trimmed document cannot merge with replicas that
   hold pre-trim history, and that Loro does not signal it reliably (empty `pending`, a 159-byte
   "update"). The wipe needs an explicit epoch or marker (ADR 0010 §8) that sync checks before
   exchanging updates. A `ShallowSnapshot` at the current frontiers keeps all `NodeId`s,
   tombstones, root registers and the peer counter, and is 31% of the full snapshot here.
10. **Verification S6:** reword to "meta map id derivable from `TreeID::associated_meta_container()`;
    nested containers of deleted nodes resolve through the op parent chain, not through
    `get_path_to_container`."
11. **§1 on `UndoManager`:** add that it also re-creates a deleted node under a new `TreeID`; the
    rejection of option A stands on one more ground.

## Not verified

- WebAssembly timings (all numbers are native release, no LTO).
- Real-world documents with curves, text, groups, many peers; the fixture is rectangles and
  4-anchor paths with translate steps.
- Concurrent undo against a peer's step on the same field (that is engine logic, not a Loro
  primitive).
- `FractionalIndex::new_between_jitter` for a fresh z-position on revival.
- Memory of a long-lived history cache beyond the first checkout (+48 MB at 154,000 ops).
