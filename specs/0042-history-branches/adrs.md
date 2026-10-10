# ADRs for "History branches"

Branches are not stored: they fall out of the `0020` stack machine run over the log. Preview reads a
forked copy of the document; Live and Clone are restore-engine writes. **No new crate, no new
dependency, no new register, no `format_version` bump.** The persisted parts it relies on are
[ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) (`Proposed`, `needs-customer`). Builds
on `0020` and `0041` merged.

## Depends on

- [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) §1 (state at a version), §2 (header
  keys `v`, `o`; continuation), §3 (step ids), §6 (stack machine, branches, main-line order),
  §8 (previews on a fork); questions 6 (go to version over other people's steps) and the
  Document-scope rule behind spec Q3 (`revert_to` reverts everybody, ADR 0014 option C).
- [ADR 0009 §1, §2, §4](../../docs/adr/0009-concurrent-editing-semantics.md): the log only grows;
  a preview is local view state; reads run against an immutable version.
- [`0020` adrs.md](../0020-undo-redo/adrs.md) decisions 2, 5, 9; [`0041`
  adrs.md](../0041-object-history/adrs.md) decisions 1 to 5.
- The blue-and-black preview rule of `specs/0009-unified-object-editing/` (criterion 17).

## Feature-local decisions (2026-10-10)

1. **Branches are derived** (1 to 6). The stack machine, fed one peer's steps in that peer's order
   (Document scope) or one object's timeline (Object scope), leaves the redo side's steps as a
   branch whenever a do step arrives; the fork point is the top of the undo side at that moment.
   Branch id = its newest step's id (3). Redo at a fork brings back the most recently left branch,
   which is what a plain stack does (2, spec Q1 A). Nothing about branches is saved (6).
2. **Peers** (5): a step of another peer is placed on the main line by (Lamport, peer) and never
   enters this peer's machine, so it cannot start a branch; each peer's own undo marks produce that
   peer's branches.
3. **Version of a step** = the frontiers of its last op ("right after step S"); before = the deps of
   its first change. Every row therefore has two exact versions, the same on every replica.
4. **Go to version** (10): read the object at the version and write it with no three-way check (a
   deliberate return, 15), header `go_to_version;s=<n>;o=<NodeId>;v=<StepRef>`. With ADR 0014 Q6 B
   it is refused when a later step on the object is by someone else.
5. **Exploring** (11 to 13) uses the continuation of ADR 0014 §2: each further press on the same
   object, with nothing committed in between, repeats the header with the new `v`. One step, one
   stack entry, one row; its before-version is that of the first press, so Ctrl+Z and Restore (12)
   are a plain undo of it. Selecting another object, switching tab or any other edit ends it,
   because the next commit has a new `s` (13).
6. **Preview** (17 to 21): on the first press, `Document::preview_at(version)` makes one fork of
   the live document (`fork()`, then `checkout(version)`), wrapped in a read-only
   `HistoricalDocument` that exposes the object reads the renderer needs and no editing method.
   Further rows only `checkout` that fork. It is dropped when the preview ends. The live document
   keeps committing and merging; "N new steps since" compares its frontiers with the fork's start.
   Nothing is written or replicated (20).
7. **Clone version** (22 to 24): read the object at the version (for a Delete row: at the step's
   before-version), create a new node with that state, fresh anchor ids from the caller's minter,
   and `clone_of = "<source>@<version>"` (`0041` decision 5), in one step
   `clone_version;s=<n>;o=<source>;v=<StepRef>`. Inherited rows follow the causal past of that
   version, so a branch's clone inherits that branch's path and not the others (23). Groups wait
   (25, spec Q5 A).
8. **Lanes** (8, 9) are presentation: `ui-core/src/history_lanes.rs`, a pure function from rows,
   statuses and fork points to lane indices (main line plus three, the rest folded).

## Placement

`curvyo-document-core/src/history/`: branch output of `stack_machine.rs`, `preview.rs`
(`HistoricalDocument`), go-to and clone writes in `restore.rs`; `curvyo-render-core`: drawing a
`HistoricalDocument` object as the blue preview; `curvyo-ui-core`: `history_lanes.rs`, preview chip
and banner texts; `curvyo-editor-wasm`: preview and Live state in `session/history.rs`; frontend:
lanes, switch, Restore, Clone buttons.

## Milestones (one branch `story/history-branches`, one PR)

1. Branch derivation and row states, lanes and keyboard (1 to 9). Architect review of the branch
   diff (public history API).
2. Preview, object and document (14, 16 to 21). Measure 21 here.
3. Live: go to version, exploring, Restore (10 to 13, 15).
4. Clone version, also from the Document scope and of deleted objects (22 to 27).

## What can start now and what waits

Nothing before `0041` merges. Milestones 1 and 2 need no customer answer. Milestone 3 runs on the
spec's Q2 A (Preview default, Live by switch) and ADR 0014 Q6 A. The Document scope gets no Live
mode (spec Q3 A) because a document-wide go-to would be `revert_to`, which reverts every peer.

## Risks

- A fork per preview doubles the document's memory while previewing; `fork()` of a large document
  may cost more than criterion 21's 300 ms for the first frame. Measure in milestone 2; fallback:
  keep the fork across previews and update it with `import` of the live document's new updates.
- Checkout distance grows with the age of the previewed row; the 10,000-step budget is measured,
  not assumed.
- Lamport order for concurrent steps can differ from wall-clock order; the list shows times, so a
  row may look out of order by a few seconds. Accepted: the same order on every replica matters
  more.
