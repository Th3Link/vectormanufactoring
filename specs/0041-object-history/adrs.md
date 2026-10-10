# ADRs for "Object history"

The object timeline is a filtered view of the step log. Object undo, object redo and inherited
history use the restore engine of `0020` with a different write rule. Two optional keys on an
object carry the wipe floor and the clone origin. **No new crate, no new dependency, no
`format_version` bump** (ADR 0014 Q2 and Q7, default A). The keys, the object wipe and reverting
other people's steps are [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md), `Proposed`,
`needs-customer`. Builds on `0020` merged.

**Amended 2026-10-10 after the Loro spike** (`docs/spikes/loro-history-primitives.md`, PR #85):
decisions 1, 2 and 6 and the risks changed.

## Depends on

- [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) §1 (restore engine on `diff`), §2
  (header keys `ou`, `od`, `o`), §5 (touched objects from the JSON ops), §6 (stack machine), §7
  (`history_floor`, `clone_of`), §10 (object wipe hides and cuts), §11 (revival with the same
  `NodeId`), §12 (checkout guard), §13 (budget of criterion 35); questions 2, 4, 6 and 7.
- [ADR 0009 §1](../../docs/adr/0009-concurrent-editing-semantics.md): option C (reverting a
  collaborator's change) is realised here as a labelled step, never on Ctrl+Z (criterion 13).
- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md): `NodeId` is
  permanent, so an object's timeline is keyed by it and an object removed by an object undo comes
  back with the same id (criterion 12).
- [`0020` adrs.md](../0020-undo-redo/adrs.md) decisions 1, 2, 5, 7, 9, 13 and 14.
- `0023-groups` for criteria 18 and 27 (the group's subtree): those criteria wait for it.

## Feature-local decisions (2026-10-10)

1. **Timeline = filter, built once.** `history/object_index.rs` maps `NodeId` to the steps that
   touched it, built in one pass over `export_json_updates_without_peer_compression` (tree op
   targets plus the child-to-parent container map, so deleted objects' anchors resolve too) on the
   first Object-scope request and extended per commit and per import from that change's ops only
   (criterion 35: no work on the commit path beyond appending). Spike: about 90 ms warm and 220 ms
   cold at 25,000 steps. The first build runs behind the long-operation state (ADR 0014 §13); after
   it the 100 ms of criterion 35 holds. A step that touched N objects is in N lists (21). Group
   timelines (18) union the current subtree's lists once `0023` exists.
2. **Object undo is a new step** (8): restore engine restricted to the object for *Change* and
   *Create* steps, the whole step for *Replace* steps (6), same per-field conflict rule as `0020`
   (7), values from `diff` between the step's versions. An object that comes back (object undo of
   a delete, object redo of a create) is revived with its `NodeId` by ADR 0014 §11 (12). Header
   `object_undo;s=<n>;t=<sec>;ou=<StepRef>;o=<NodeId>`; object redo `…;od=…`. It is an ordinary
   entry of my stack, so Ctrl+Z on it is a plain undo of that commit, which is the object redo of
   criterion 9. Nothing else is special-cased.
3. **Which step is "latest applied" and what is redoable** come from the stack machine of `0020`
   run over this object's timeline with the `ou`/`od` marks (11: a new do step on the object clears
   the object's redo side).
4. **Whose steps** (13): any author by default (ADR 0014 Q6 A). With B, the machine filters the
   object's timeline to the current peer before choosing.
5. **Clone origin.** Duplicate and Ctrl-copy write `clone_of = "<source NodeId>@<version>"` on each
   copy in the same commit; `version` is the encoded frontiers at the duplication. The copy never
   inherits the source's `history_floor` or `clone_of` (the meta-map copy in `objects.rs` drops
   both before writing its own). Inherited rows (24) = the source's timeline restricted to the
   causal past of `version`, from the source's floor as it was at that version, recursively
   through the source's own `clone_of` (26). They are computed on request, never copied, so later
   steps of either object cannot leak into the other (24, 33).
6. **Undo into inherited rows** (25) is a state restore, not an inverse: read the source at the
   inherited step's before-version through the checkout guard of ADR 0014 §12 (one object, so the
   cost is the checkout distance), write those fields into the clone as one step on the clone. The
   source is never written. Above 1,000 steps back it runs behind the long-operation state.
7. **Object wipe** (29 to 34): a step `wipe_object_history;s=<n>;t=<sec>;o=<NodeId>` that writes
   `history_floor` = the encoded current frontiers. The timeline hides every step in the causal
   past of the floor; a concurrent peer step stays visible. Not pushed on any stack; my stack drops
   entries that touched only this object (31). The floor register is the op that makes the step
   exist, and an O(1) read for the timeline.
8. **Boolean, Combine, Break apart, Offset results** are fresh nodes without `clone_of` (20, 28,
   Q2 A). Their first row names the sources from the step's touched objects.
9. **Version encoding** for both keys: sorted `peer:counter` pairs joined by `,`, parsed leniently:
   an unreadable value means "no floor" or "no origin", never a refused file.

## Placement

`curvyo-document-core/src/history/`: `object_index.rs`, `lineage.rs` (clone origin and inherited
rows), floor read and write next to the meta-map codec (`objects.rs` duplicates without the keys);
`curvyo-ui-core`: object-scope rows and notices in `history_rows.rs`; `curvyo-editor-wasm`:
`session/history.rs` gains Ctrl+U, Ctrl+Shift+U, wipe; key rows in `session/keys.rs`; frontend:
the Object scope of the History tab.

## Milestones (one branch `story/object-history`, one PR)

1. Object index, timeline rows, object scope list (1 to 4, 21, 22, 35). Measure 35 here, cold and
   warm, and once on wasm.
2. Object undo and redo, notices, buttons (5 to 17). Measure object undo of a step 1,000 steps
   back (spike: 103 ms with the checkout pair; `diff` between two old versions is unmeasured).
3. Clone origin and inherited rows (19, 20, 23 to 28 without groups). Architect review of the
   branch diff: the two keys are the document-model change.
4. Object wipe (29 to 34).
Criteria 18 and 27 (groups) join the branch when `0023` has merged, or move to a follow-up.

## What can start now and what waits

Nothing before `0020` milestone 2 merges. Milestones 1 and 2 need no customer answer beyond
ADR 0014 Q6 and Q7 (defaults A). Milestones 3 and 4 write the keys: they run on ADR 0014 Q2 A and
Q4 A and merge only with those answered or the defaults accepted. Ctrl+Shift+U on WebKitGTK (spec
Q5) is checked by hand in milestone 2.

## Risks

- First-request index build: measured, not seconds (220 ms cold at 25,000 steps). The fallback of
  building it in chunks in idle callbacks is not needed unless wasm is much slower.
- An older build duplicating an object copies `clone_of` verbatim: a newer build then shows the
  grandparent's lineage. Never a wrong drawing (ADR 0014 consequences).
- Restore with the same `NodeId` (criterion 12) depends on the injected tree move of ADR 0014 §11
  and its canary; with Q7 B it is a move out of the trash node instead.
