# ADRs for "Undo and redo"

Undo is our own restore engine over Loro versions, steps are delimited by a header in the commit
message, the stacks come from one pure stack machine, and the history list is derived from the log
`document.loro` already holds. **No new crate, no new dependency, no `format_version` bump.** The
persisted parts (commit-message grammar, times, wipe) are
[ADR 0014](../../docs/adr/0014-history-undo-and-branches.md), `Proposed`, `needs-customer`.
Reference state: `main` at `52d4101`.

## Depends on

- [ADR 0002 §5, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md): one commit
  per interaction, the log is the history, `NodeId` is minted once and survives an undo of Delete.
- [ADR 0009 §1, §2](../../docs/adr/0009-concurrent-editing-semantics.md): peer-scoped, forward
  commit, never resurrect a peer's deletion; selection is view state. Criteria 15 to 21.
- [ADR 0004 §1, §3, §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md): history lives in
  `document.loro`; no Loro type in any public API; format rules.
- [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) §1 to §7 and §9: this spec is its
  first user. ADR 0014 §1 supersedes the "Loro's undo manager is the mechanism" sentence of 0004 §2.
- [ADR 0010](../../docs/adr/0010-document-keyring-admins-and-revocation.md) §8: why a wipe is not
  offered once a keyring exists (criterion 50).
- [`0044` adrs.md](../0044-editing-quick-wins/adrs.md) decision 4: its held-nudge requirement is met
  by a continuation (decision 2 below), not by a `translate_objects_continued` label.

## Feature-local decisions (2026-10-10)

1. **Engine, not `UndoManager`** (ADR 0014 §1). Loro's `UndoManager` has no per-object veto, so
   criteria 17 to 19 cannot be built on it. Undo of step S reads each touched object at S's before
   and after versions and writes back field by field where the current value is still the one S
   wrote (criterion 16), removes an object S created only if nobody changed it (18, default A),
   skips objects a peer deleted (17) and restores objects S deleted with their old `NodeId`. Redo is
   the same with before and after swapped. The write is one commit with header
   `undo;s=<n>;u=<StepRef>` (or `redo;…;d=…`), replicated like any edit (20). The restore also
   covers root registers (document size, unit) for *Document* steps.
2. **Step boundaries** (ADR 0014 §2). `commit_with_label` writes `<label>;s=<seq>` with the
   session's counter and the time set by `Document::set_step_time(Timestamp)` (seconds; a newtype
   in `units.rs`). Large commits that Loro splits stay one step (criterion 14); two commits never
   merge into one (13): steps are grouped by header, not by Loro change, so the merge interval
   does not matter. `Document::continue_step()` makes the next commit join the peer's last step,
   refused when the document version moved since. That is 0044's held nudge.
3. **Gestures with several commits become one** with `Document::batch(label, |doc| …)`: inner
   `commit_with_label` calls are deferred, one commit at the end. Introduced only where the commit
   path audit (milestone 1) finds a gesture with more than one commit. Candidates to check:
   multi-object resize and rotate after `0019` (`ui-core/transform_commit.rs` writes per object),
   Ctrl-copy move, typed entries that write two registers.
4. **`Document::new` commits its root initialisation** with the reserved label `new_document`.
   Today those ops ride along in the first user commit, so undoing the first step would remove
   `format_version` and the size. A `new_document` step is listed nowhere and never stacked.
5. **One stack machine** (ADR 0014 §6), `document-core/src/history/stack_machine.rs`, pure:
   events do / undo / redo / continuation, 500 entries (criterion 28, a constant, YAGNI). The
   session feeds it its own actions; New and Open make a new `Document` with a new peer, so empty
   stacks (25, 31) need no code. A press pops exactly one entry even when everything is skipped
   (19). Remote and earlier-session steps never enter it (15, 21).
6. **Selection per step** is held in `ui-core/src/undo_stack.rs`, keyed by `StepRef`: selection and
   node selection before and after (23). Pan and zoom are not touched (24).
7. **Step data** (criteria 32 to 36): `StepRef` and the shown id per ADR 0014 §3; author "You" when
   the peer equals the document's current peer, else "Earlier session"; time from
   `ChangeMeta::timestamp`, 0 = "Unknown time"; kind and touched objects derived from the ops (ADR
   0014 §5), with the object kind ("rectangle") read at the version where the object existed.
   Legacy messages (no `;`) are steps by label; unknown labels show "Edit" (36).
8. **Operation names** live in `ui-core/src/history_names.rs`: one table label → name; the guard
   test of criterion 41 greps `commit_with_label("…")` and `*_COMMIT_LABEL` in the sources.
   Reserved labels `new_document`, `undo`, `redo`, `wipe_history` are in the table as not-a-row.
9. **List source**: `Document::history_page(before: Option<StepRef>, count)` walks the log newest
   first (`travel_change_ancestors`), groups changes into steps and returns rows; touched objects
   are computed for the returned rows only and cached by `StepRef` (criterion 43: 50 rows).
   "Undone" status comes from the stack machine run over each peer's steps (39).
10. **Persistence** (37): no change. Save already writes the full log. Golden fixture: a file with
    header messages and times, reopened, rows equal.
11. **Wipe** (47 to 51, ADR 0014 §9): `Document::wipe_history(time)` exports a shallow snapshot at
    the current frontiers, re-imports it into the same `Document` under the same peer, then commits
    a step writing the root key `history_wiped` (the step needs one op to exist). The session does
    not offer it for a shared document (one with a `keyring.log`; none exists before the sharing
    slice, so today the check is a constant the sharing slice replaces). Golden fixture `wiped_v9.curvyo` (the number is whatever `main`
    has at merge; the wipe itself does not bump).
12. **Merge seam for tests** (15 to 21): `Document::export_updates(since: &DocumentVersion)` and
    `import_updates(&[u8])`, bytes only, validating the tree after import like `unpack`. The sync
    story reuses them.

## Placement

`curvyo-document-core/src/history/` (new module directory, one responsibility per file:
`step_header.rs`, `step_id.rs`, `step_log.rs`, `touched.rs`, `stack_machine.rs`, `restore.rs`,
`wipe.rs`); `curvyo-ui-core`: `undo_stack.rs`, `history_names.rs`, `history_rows.rs` (summaries,
notices); `curvyo-editor-wasm`: `session/history.rs` (commands), key rows in `session/keys.rs`
(the one gate of `0010`), `wasm_history.rs`; `curvyo-app`: the native Edit menu (Q5); frontend:
History tab (after `0043`).

## Milestones (one branch `story/undo-redo`, one PR)

0. Spike `spike/loro-history-primitives` (ADR 0014 "Verification"), never merged; needs disk space.
1. **Engine**: step header, step time, `new_document` commit, commit-path audit with one test per
   table row (10, 41), `batch` where needed, restore engine, stack machine, keys, gate, conflict
   rules, selection, limits, the "no undo" texts (1 to 31, 41, 52). Architect review of the branch
   diff here (document model, public API). Ctrl+Z works end to end.
2. **Step data**: ids, time, author, kinds, legacy files (32 to 38).
3. **History tab** (39 to 46), after `0043` merges.
4. **Wipe** (47 to 51).

## What can start now and what waits

Milestones 0 to 3 can start on ADR 0014's defaults: Q1 A is today's behaviour, the header grammar
and no-bump are Q2 A, authors are Q5 A. Milestone 4 builds on Q3 A (unshared only). The PR merges
only after the customer has answered or accepted the defaults. If S1 (restore a deleted node with
its id) fails, milestone 1 stops and the architect brings a soft-delete design to the customer.

## Risks

- Loro API names come from docs.rs, not the source: the spike pins them before milestone 1.
- Two checkouts per undo on a 5,000-object document may miss criterion 30. Measured in milestone 1;
  fallback: a shadow fork kept in sync by updates (double document memory).
- Touched-object derivation for nested containers of deleted objects (spike S6).
- Old files: adjacent same-label commits show as one row; the first row includes the root
  initialisation; no times. Stated in the release notes.
- Criterion 29 (50 MB): stacks hold refs and selections only; the log itself is the memory cost
  and is unchanged by this slice.
