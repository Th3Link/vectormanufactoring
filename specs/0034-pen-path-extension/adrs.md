# ADRs for "Pen path extension"

The Pen gains three press targets (continue, join, close with a join type),
and the document gains three labelled commands that grow one existing path.
**No new crate, no new dependency, no new ADR, no `format_version` bump, no
trait, no generic.** The anchor schema is unchanged: every write is an
ordinary anchor insert, delete or register write. Reference state: `main` at
`909a2cc`.

## Depends on

- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one gesture, one commit (`extend_path`, `connect_paths`, `close_path`).
  Every id is resolved before the first write, and a refusal writes nothing.
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md): the
  continuation, the hover target and the closing preview are ephemeral Pen
  state. The continued path is not touched until the commit (criterion 25).
  The absorbed path's anchors move by delete plus insert, with the same
  delete floor as Join.
- [ADR 0001 §1, §3, §4](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  targets, join resolution and command plans live in `ui-core`.
  `render-core` draws the cue from data. `editor-wasm` binds.
- [`specs/0002-path-node-editing/adrs.md`](../0002-path-node-editing/adrs.md):
  "a pen session is one commit", "commands carry resolved geometry", and one
  resolving function for both preview and commit (`resolve_anchor`,
  `is_close_target`).
- [`specs/0006-path-merge-split-and-node-types/adrs.md`](../0006-path-merge-split-and-node-types/adrs.md):
  Join's reversal rule (reverse the list, swap `handle_in`/`handle_out`), the
  coincident-merge rule (midpoint, each side's inward handle, Corner), and
  Corner to Asymmetric conversion (criterion 2).
- [`specs/0016-boolean-operations/adrs.md`](../0016-boolean-operations/adrs.md):
  compound paths leave the node tools at `Session::paths()`. Here they are
  never targets.

## Feature-local decisions

- **2026-10-10: three shared pure helpers, extracted rather than copied.**
  Whichever of 0034 and 0035 merges first adds `reversed_anchors`; the other
  rebases onto it.
  - `curvyo-document-core::reversed_anchors(&[NewAnchor]) -> Vec<NewAnchor>`
    (in `path_model.rs`): the list reversed, with each anchor's handles
    swapped. `join_two_objects` calls it instead of its inline loop. 0034 uses
    it to prepend and to connect; 0035 uses it to set windings.
  - `curvyo-document-core::smooth_corner_handles(prev: Point, next: Point,
    handle_in: Vec2, handle_out: Vec2) -> (Vec2, Vec2)`, in a new small
    module `smooth_handles.rs`, holds the Corner to Asymmetric rule that is
    inline in `convert_anchor_kind` today: tangent from `prev` to `next`,
    each handle keeping its length or taking `DEFAULT_HANDLE_LENGTH_MM`.
    `convert_anchor_kind` calls it. Criterion 15's "Smooth" is exactly this
    rule, with the closing node's two neighbours in the closed list.
  - `curvyo-document-core::merged_junction(...) -> AnchorSnapshot` holds the
    0006 merge rule that is inline in `join_same_path` and
    `join_two_objects`: midpoint, each side's inward handle, Corner, and the
    id of the surviving side. Both Join functions call it. 0034 uses it for
    criterion 9 (ends within 0.001 mm) and criterion 19 (first and last node
    within 0.001 mm).
- **2026-10-10: three new commands in a new module
  `curvyo-document-core/src/path_extend.rs`.** `paths.rs` (1463 lines) and
  `path_topology.rs` (1141) are past the module limit. `join_endpoints` does
  not fit: it always merges two nodes into one, and criteria 8 and 9 add a
  segment instead. The commands take one resolved description:

  ```rust
  pub enum PathEnd { First, Last }
  pub struct PathGrowth {
      pub path: NodeId,                       // the surviving object
      pub end: PathEnd,                       // where `added` (and `absorb`) attach
      pub added: Vec<NewAnchor>,              // new anchors, stored order
      pub absorb: Option<(NodeId, PathEnd)>,  // connect: the other path and its joined end
      pub replace: Vec<AnchorSnapshot>,       // resolved values of existing anchors that change
      pub drop: Vec<AnchorId>,                // existing end anchors removed by a merge
  }
  pub fn extend_path(&self, growth: &PathGrowth) -> Result<(), PathEditError>      // "extend_path"
  pub fn connect_paths(&self, growth: &PathGrowth) -> Result<(), PathEditError>    // "connect_paths"
  pub fn close_paths(&self, growths: &[PathGrowth]) -> Result<(), PathEditError>   // "close_path"
  ```

  All three run one private uncommitted funnel. It resolves and checks
  everything first: the survivor exists, is ordinary (not compound) and
  open; the absorbed path is a different ordinary open path; added ids are
  unique and unused; `replace` and `drop` ids are anchors of the survivor or
  of the absorbed path; and a closed result has at least 3 anchors. Then it
  drops, inserts `added` at `end`, moves the absorbed anchors in after them
  (reversed with `reversed_anchors` when both joined ends are the same kind
  of end, as Join does), writes `replace`, deletes the absorbed object, sets
  `closed` (only `close_paths` does), and commits once with the label of the
  public command. `extend_path` refuses `absorb`; `connect_paths` may omit
  it (a new path that ends on Q grows Q); `close_paths` writes all its paths
  in one commit (criterion 19). New `PathEditError` variants, each named by
  its reason: `NotExtendable` (closed, compound, or the same object) and
  `TooFewToClose`. Duplicate ids get their own variant. The
  survivor's style, id, z-place and the ids of its anchors are never written
  (criteria 5, 10, 20). Its node order never changes (criteria 4, 8).
  Rejected: a general `set_path_anchors(path, list)` that diffs by id. It
  is more code, it writes untouched anchors under concurrent edits, and its
  only callers are these three.
- **2026-10-10: the Pen keeps its path in drawing direction; the reversal
  happens at commit.** In `curvyo-ui-core/src/pen_tool.rs`, `State::Placing`
  gains `base: Option<Continuation>`.
  `Continuation { path, end: PathEnd, original: Vec<AnchorSnapshot> }` is P as
  read at the press. The Pen's node list starts with E, with E's handles in
  drawing direction (swapped when `end` is `First`), so `build_pen_preview`
  draws the rubber band from E and E's permanent ring without a change
  (criteria 2, 3). At commit, the new nodes (everything after E) become
  `added`, passed through `reversed_anchors` when `end` is `First`
  (criterion 4). `escape` already drops the state, which is criterion 25.
  `set_tool` already calls `finish` and then `escape`, which is criterion 26.
  `PointerDown.closing: bool` becomes `action: PressAction` with variants
  `Place`, `Continue`, `Join`, `Close(JoinType)`. The action is decided at
  the press, as `closing` is today, and runs at the release. A drag is
  ignored for every target except `Place`. If a commit is refused (P or Q
  deleted under the Pen, which cannot happen in a single-user session), the
  Pen drops its state and writes nothing.
- **2026-10-10: one target function for the hover cue and the press, in a new
  module `curvyo-ui-core/src/pen_target.rs`.** `pen_target(pen, index, point,
  radius, shift) -> PenTarget`, with variants `Place`, `NewPathAt` (Shift
  over an end), `Continue{path, end}`, `Join{path, end}`, `PlaceOverEnd`
  (Shift over a join target) and `Close{join, shift}`. The session derives
  the cursor class, the chip code and the ring position from this one value,
  so the cursor, the chip and the press always agree (criterion 22). Chip
  sentences live in the frontend, keyed by code, as `booleanText.ts` does.
  The radius is the existing close radius (`point_tolerance_as_length`,
  16 px; criterion 23). Priority:
  1. The in-progress path's own close target. For a new path this is its
     first node, for a continuation it is P's other end F. Criterion 13
     requires at least 3 nodes when closed.
  2. End nodes of other open ordinary paths. While continuing, both ends of
     P are excluded (F is rule 1; E is the node being drawn from). The
     nearest end wins; a tie goes to the topmost object, then to the last
     node (criteria 7, 12).
  3. Place a node.

  While the Pen is idle, only rule 2 applies, with `Continue` in place of
  `Join` (criterion 1). There is no selection-handle layer in the Pen to
  yield to.
- **2026-10-10: the closing join is one pure function, in a new module
  `curvyo-ui-core/src/closing_join.rs`.** `JoinType::resolve(kind, shift)`
  gives "as drawn" or flipped (criterion 15). `resolve_closing_node(prev,
  closing, next, join) -> AnchorSnapshot` applies Sharp (kind becomes Corner,
  handles kept) or Smooth (`smooth_corner_handles` for a Corner node;
  unchanged otherwise). Three callers use it: the Pen preview (criterion
  17), the Pen commit (a new path goes through `create_path(nodes, true)`
  with the resolved first node; a continuation goes through `close_paths`
  with the resolved F in `replace`), and the Close path command
  (criterion 19). Preview and commit therefore cannot disagree.
- **2026-10-10: Close path command. `curvyo-ui-core/src/close_path.rs`.**
  `close_path_available(paths, ids) -> bool` and `plan_close_paths(paths,
  ids, join) -> ClosePlan { growths, closed, skipped }`. When the ends are
  within 0.001 mm, the plan puts `merged_junction` into `replace` and the
  last node into `drop`, then applies the join (criterion 19). Compound
  paths, primitives and closed paths are not counted (criterion 21). The
  Select tool's object selection and the Node tool's editing set call the
  same functions. The two bars' state structs (`SelectBarState`,
  `NodeToolbarState`) each gain one flag. Where the buttons sit is decided
  in the UX notes. The notice is a code plus counts; the sentence lives in
  the frontend (criterion 20).
- **2026-10-10: hover performance (criterion 24). Cache the end nodes by
  document version.** `Session::paths()` reads every path out of Loro, so
  5000 paths per pointer move would take far longer than 2 ms (`docs/technical-debt.md`,
  "Canvas performance": 12 ms for 200 objects). `pen_target.rs` holds
  `EndNodeIndex`, a flat `Vec` of the end nodes of open ordinary paths
  (point, path, end, z), built from `Session::paths()`. A query scans it
  linearly: 10 000 points take microseconds. The session caches
  `(DocumentVersion, EndNodeIndex)` in a `RefCell` and rebuilds it when
  `Document::version()` differs. `Document::version()` is a new read-only
  accessor over Loro's state frontiers, as an opaque `PartialEq` type. It is
  the accessor the draw-list cache item in technical debt asks for, so the
  PR updates that item. Budget: an `#[ignore]` release benchmark in
  `curvyo-editor-wasm/tests/` with 5000 open paths: hover query ≤ 2 ms
  with the cache warm; the rebuild after a commit is reported separately.
  Rejected: a spatial index (not needed at this size).
- **2026-10-10: rendering.** The cue goes in a new `render-core` module
  `pen_cue.rs`: the hover ring at the target node, and the dashed closing
  segment with the resolved closing node's handles (criterion 17).
  `session/draw.rs` calls it after `build_pen_preview`. `pen_preview.rs`
  (540 lines) does not grow. The two new cursors are frontend assets
  (`lib/cursors`, `Canvas.tsx`).
- **2026-10-10: format impact: none.** The three commands write existing
  registers and anchor maps. The labels `extend_path`, `connect_paths` and
  `close_path` are new commit messages, pinned by a test (as `boolean_command.rs`
  does).
- **2026-10-10: no undo (criterion 27).** Each gesture is exactly one commit.
  Escape before the finish is the only way back, and it writes nothing.
  Tests count commits with `len_changes()` on small inputs only (see the
  technical-debt item "one commit is not one Loro change").

## Files shared with other slices

- **0017**: `editor-wasm/src/session/draw.rs` and `session/mod.rs`,
  `document-core/src/paths.rs` (0034 moves the Corner to Asymmetric math out
  of `convert_anchor_kind`), and the `lib.rs` export lists of `document-core`,
  `ui-core` and `render-core`. These are rebase-level conflicts. 0034 does
  not touch the style codec, the gradient code, the `ui-core` style modules
  or the panel.
- **0031**: `ui-core/src/node_tool.rs` (0034 adds one `NodeToolbarState`
  flag; 0031 adds `Drag::Bend`), `editor-wasm/src/session/draw.rs`,
  `session/mod.rs`, `session/node.rs` / `wasm_node_tool.rs` (the Close path
  binding), and the `lib.rs` export lists. 0034 does not touch
  `geometry-core/src/segment.rs`, `segment_bend.rs`, `hit_test.rs` or
  `render-core/src/decorations.rs`. Order: 0034 starts after 0031 merges
  (build-order waves 3 and 5).
- **0035**: `path_model.rs` (`reversed_anchors`, see above), the
  `ui-core`/`document-core` `lib.rs` lists, and `session/mod.rs`. Both slices
  touch `document-core`, `ui-core`, `editor-wasm` and the frontend, so under
  `CLAUDE.md` §4 they do not run in parallel. Recommended order: 0034 first.
  It needs no rail decision, and 0035 waits for the UX rail design.

## Milestones on one branch (`story/pen-path-extension`)

1. `document-core`: the three helpers, `path_extend.rs`, `PathEnd`,
   `Document::version()`. Tests for criteria 4, 5, 8 to 11, 19 to 21 at
   document level, plus the labels and refusals.
2. `ui-core`: `closing_join.rs`, `pen_target.rs`, `pen_tool.rs`
   (`Continuation`, `PressAction`), `close_path.rs`, the bar flags. Criteria
   1 to 4, 6, 7, 12 to 16, 18, 23, 25, with property tests showing that
   preview and commit give the same result.
3. `render-core` + `editor-wasm`: `pen_cue.rs`, the index cache, the cue
   codes, bindings, and the benchmark (criteria 17, 22, 24, 26).
4. Frontend: cursors, chip texts, the Close path buttons per the UX notes,
   the notice. Then the UX review and the tester pass on the whole slice.

## Flagged to the PO (defaults taken)

1. Criterion 24: the 2 ms holds for a move with the cache warm. The first
   move after a document change rebuilds the index with one document read
   (tens of ms at 5000 paths, the same read every frame already pays).
   Default: report both numbers in the PR.
2. Criterion 13 allows closing a continuation with no new node when P has 3
   or more nodes (the closing segment then runs from E to F). It is built
   that way.
