# ADRs for "Editing quick wins: select all and keyboard nudge"

Two new rows in the existing key table and a reuse of the typed move's write;
a held key joins one step through ADR 0014's continuation. **No new ADR, no
new commit label, no new crate, no new
dependency, no format bump, no trait, no generic.** Reference state: `main` at
`52d4101` plus #80 (`story/multi-object-transform`), which this slice builds
on.

## Depends on

- [ADR 0002 §4, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  document coordinates in mm, Y down; every write is a labelled commit.
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): select
  all is view state and writes nothing (criterion 4).
- [`0010` adrs.md](../0010-edit-interaction-polish/adrs.md) decisions 4 and
  6: one key table and one gate, `session/keys.rs::decide`, a pure function.
- [`0019` adrs.md](../0019-multi-object-transform/adrs.md) (#80): the move of
  several objects is one `translate_objects` commit; the group box gives the
  selection's bounds.
- [ADR 0014 §2](../../docs/adr/0014-history-undo-and-branches.md) (Proposed)
  and [`0020` adrs.md](../0020-undo-redo/adrs.md) decision 2: a step is the
  commits of one peer with the same header `<label>;s=<seq>`; a continuation
  repeats the previous step's header and joins it, allowed only while the
  document version is still that step's last version
  (`Document::continue_step()`). Decision 4 uses it.

## Feature-local decisions (2026-10-10)

1. **Ctrl+A is a row in `decide`, before the generic "Ctrl means ignore"
   line.** It acts when: key `a` (any case), Ctrl or Cmd, no Alt, no Shift,
   not a repeat, `dom_blocked` false, the Select tool, no drag in flight, no
   entry chip open, no unfinished Pen path. Otherwise `Ignore` (no
   `preventDefault`, criterion 3). The action is
   `ObjectSelection::set(&document.object_ids())`: the root's objects in
   z-order, bottom to top (criterion 1's "stacking order"). 0023 replaces
   `object_ids()` with the entered group's children; 0039 filters hidden and
   locked objects. An empty document still returns a handled outcome
   (`"select-all"`, nothing changes) so the page's own select-all does not
   highlight the UI's text. New `KeyOutcome::SelectedAll`.
2. **Nudge is a row in `decide` for `ArrowLeft/Right/Up/Down`.** It is the
   only row besides Escape that accepts `repeat`. Gate: the Select tool, at
   least one object selected, no Ctrl, Cmd or Alt, `dom_blocked` false (a
   focused field, button, the 0043 tab strip or the rail keeps its arrows,
   criterion 11), no drag, no chip, no unfinished Pen path. Otherwise
   `Ignore`. The distances are two `Length` constants in a new
   `curvyo-ui-core/src/nudge.rs`: `NUDGE = 1 mm`, `NUDGE_LARGE = 10 mm`
   (Shift). Right is +x and Down is +y. Zoom and display unit are not
   inputs (criterion 7).
3. **The write is the typed move's write.** `nudge.rs` resolves
   `(direction, shift) -> Vec2` and checks the coordinate limit; the session
   calls `transform_commit::commit_move(document, ids, offset, false, minter)`,
   which is `Document::translate_objects`. So compound paths, primitives,
   paths and several objects behave exactly as with `M` and a drag
   (criterion 8). The limit check is the one in `MoveEntry::resolve` (every
   edge of the tight bounds plus the offset within `MAX_COORDINATE_MM`). It
   moves into one `pub(crate) fn offset_within_limit(bounds, offset) -> bool`
   in `transform_commit.rs`, used by both, so the rule exists once. Bounds:
   `object_outline_bounds` for one object, #80's `GroupSelection` for
   several. A refusal returns `KeyOutcome::Hint(KeyHint::TooFar)`
   (`"hint-too-far"`); the frontend reuses 0019 criterion 22's text.
4. **One commit per key event; a held key joins one step (ADR 0014 §2).**
   Options:
   - *A (chosen): every key event writes at once* through the typed move's
     write (`commit_move`, label `translate_objects`, criterion 8). The first
     event of a run opens a step; each continuation repeats that step's
     header (`translate_objects;s=<seq>`) through `Document::continue_step()`
     and joins it. No special label. The document is always true: Save, a
     merge, a pointer press or a tool change at any moment sees the moved
     objects, and nothing has to be flushed.
   - *B (rejected): preview the run and commit once at its end.* Every entry
     point of `Session` (pointer, keys, panel, tool change, save, merge,
     blur) would have to flush the pending move first, and the end of a run
     needs a timer because key-up can be missed (criterion 9). One forgotten
     flush means a save without the last move.
   - *C (rejected): plain commits grouped by time.* 0020 criterion 13 forbids
     merging steps by time.

   **A continuation** is a pure decision in `nudge.rs`: the event is a
   repeat, has the same arrow and the same Shift state as the previous nudge,
   and arrives at most 600 ms after it (the DOM's `KeyboardEvent.timeStamp`,
   passed in as `KeyInput::time_ms`; core reads no clock). The other
   condition, nothing committed or merged since the previous nudge, is ADR
   0014's own guard: `continue_step()` is refused when the document version
   moved, and the event then opens a new step. Key-up needs no call: a new
   press has `repeat = false` and starts a new run.
   **Dependency on 0020:** `continue_step()` and the step header come with
   0020's milestone 1. Until 0020 exists a held nudge writes plain
   `translate_objects` commits, one per event, and the continuation decision
   in `nudge.rs` has no consumer; it is wired to `continue_step()` by
   whichever of 0044 and 0020 merges second (one call in the session). There
   is no undo before 0020, so the maker sees no difference.
   **Cost (accepted):** about 30 commits per second while a key is held,
   each rewriting the selected objects' positions; the oplog grows
   (`docs/technical-debt.md`, "Document files grow with edit history").
5. **Draw-list cache: fold the first half into this slice, as milestone 1.**
   Criterion 5 (5,000 objects, Ctrl+A drawn within 100 ms) cannot pass
   without it. Select all changes no document version, but every frame
   re-reads every object, and that read cost about 12 ms for 200 objects
   (debt entry "Canvas performance on Linux/WebKitGTK").
   Design:
   - `Session::objects()` returns `Rc<[ObjectSnapshot]>` from a cache
     `RefCell<Option<(DocumentVersion, Rc<[ObjectSnapshot]>)>>`. The cache is
     rebuilt when `document.version()` differs.
   - The existing `drag_objects` snapshot merges into the same field with a
     `pinned` flag: a Select or Node drag keeps its first read even if a merge
     arrives (unchanged rule).
   - Callers that alter objects for drawing (style preview, Node live drag)
     keep their `Cow::to_mut()` copy. The cache holds document reads, never
     previews.
   - `new_document` and `open` clear the cache explicitly. Two different
     documents can have equal frontiers (both empty, say).
   - Not now: caching the tessellated artwork by (version, scale, dpr). That
     is a second step, done only if the `#[ignore]` benchmark still misses
     100 ms after the read cache.

   Risks: a reader that expects fresh state during a pinned drag (as today);
   a forgotten clear on document replacement (a test covers New and Open).
   The debt entry is updated in the PR.

## Tests

- `keys.rs` table rows: Ctrl+A in every tool and gate state; arrows with
  every modifier; repeat accepted for arrows only; a focused field ignores
  both.
- Session: 5 objects, Ctrl+A, selection equals `object_ids()` in order, no
  commit (version unchanged). One press moves 1 mm with one
  `translate_objects` commit. 1 press plus 10 repeats moves 11 mm in 11
  `translate_objects` commits, and `nudge.rs` marks events 2 to 11 as
  continuations; with 0020, the 11 commits carry one header (one step).
  3 presses give 3 runs. A gap over 600 ms, a Shift change or another arrow
  starts a new run; with 0020, an interleaved commit does too. The limit refusal writes nothing.
  Compound path, primitive and multi-selection give the same result as `M`.
- `#[ignore]` benchmarks: 5,000 objects Ctrl+A to drawn list under 100 ms
  (release); nudge of 1,000 objects per event.

## Delivery: one branch `story/editing-quick-wins`, one PR

Starts after #80 merges (it needs `GroupSelection`, the multi-object move and
#80's `keys.rs`). It may be stacked on #80's branch until then. It does not
run in parallel with 0043 or 0020 (same `editor-wasm`/`ui-core` files).
Milestones: (1) objects cache and benchmark; (2) `nudge.rs`, the shared
limit check, the key rows, session tests; (3) frontend: pass `timeStamp`,
the `"select-all"` and `"hint-too-far"` outcomes, the shortcut-table rows.

## Flagged to the PO (defaults taken)

1. Criterion 9's pre-0020 sentence ("a step is one commit") becomes
   "before 0020 a held press writes one `translate_objects` commit per
   event; with 0020 they form one step" (decision 4).
2. Criterion 2 / 3: Ctrl+A on an empty document is handled (default
   prevented) so the page does not select the UI's text; in other tools it is
   ignored as written, which may let the webview select text. The UX review
   checks this.
