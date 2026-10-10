# ADRs for "Groups: group, nest, enter and leave, ungroup"

A group is an interior node of the object tree that ADR 0002 §5 already
decided. It needs no new ADR. It does change the document model and the file
format, which the customer accepted on 2026-10-10 (spec Question 1), and it
takes the **next free `format_version` at merge** (11 if 0040's 10 merges
first). No new crate, no new dependency, no trait, no generic.

## Depends on

- [ADR 0002 §5, §9, §11](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  the document is a tree, and sibling order is z-order. Reparenting is a tree
  move that keeps the `NodeId`. There is no style inheritance. Each command
  is one commit. The format is versioned, and a reader refuses a newer
  version.
- [ADR 0009 §1, §2](../../docs/adr/0009-concurrent-editing-semantics.md):
  the entered group (the context) is ephemeral view state, like the
  selection. It is not stored or undone, and it is resolved lazily
  (criterion 20). Undo skips a step whose group a peer removed.
- [ADR 0014 §7, §11](../../docs/adr/0014-history-undo-and-branches.md): a
  step records each touched node's tree parent and z-position as fields.
  Undo of Group or Ungroup restores them, and the injected tree move
  revives a deleted group with its own `NodeId`. 0023 adds no history code.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  no silent partial read, so a build without groups must refuse a file that
  holds one ("saved by a newer version").
- [`specs/0019-multi-object-transform/adrs.md`](../0019-multi-object-transform/adrs.md):
  the group box, its handles and `transform_objects`. A group is transformed
  by applying the transform to its leaves (criterion 22).
- [`specs/0016-boolean-operations/adrs.md`](../0016-boolean-operations/adrs.md)
  and [`specs/0035-combine-and-break-apart/adrs.md`](../0035-combine-and-break-apart/adrs.md):
  the refusal pattern (code plus offenders) that criterion 28 extends with
  `Group`.

## Feature-local decisions

- **2026-10-10: the stored shape of a group (format, protect this).** A group
  is a node of the existing object tree (container key `"paths"`) whose meta
  map holds exactly one register, `group: true`. Its children are its tree
  children, and their sibling order is their z-order inside the group. It has
  no style keys, no `rotation`, no anchors and no `shape`. Open-file
  validation checks `group` first, then `shape`, and recurses into children.
  A file is refused as damaged if a non-group node has children, a group
  node also carries anchors or a `shape` tag, or more than 32 groups
  lie on one root-to-leaf chain (criteria 5, 27). An empty group is valid on
  read (see the concurrency decision below). A Loro tree cannot hold a
  cycle, so criterion 27's "cycle" case cannot be written into a file. The
  tester covers depth only, and the PO may drop the word. Rejected: a `shape:
  "group"` tag, because every primitive match arm would then have to exclude
  it. Also rejected: a flat list with a `parent` register, because ADR 0002
  §5 and ADR 0012 (option C, lost for the same reason) already ruled it out.
- **2026-10-10: no transform and no style on a group.** ADR 0002 §5 lets a
  node carry its own transform. A group does not, and the customer accepted
  this as Question 2, A. A later group transform (Question 2, B) would be an
  amendment of ADR 0002 and a customer decision. A style edit on a group is
  expanded to its leaf descendants in `curvyo-ui-core` before the write
  (criterion 26). `document-core` never writes style to a group node.
- **2026-10-10: commands in `curvyo-document-core`, new module `groups.rs`.**
  `group_objects(members) -> Result<NodeId, _>` (label `group`) creates the
  group node in the members' common parent, directly after the topmost
  member. It then moves the members into it with tree `mov`, in their
  relative order. Leaves keep their `NodeId`s and anchor ids (criteria 1,
  4). `ungroup(groups)` (label `ungroup`) moves each group's children to the
  group's place in its parent and deletes the group node. Both resolve every
  id and check the depth before the first write. Members that do not share
  one parent are an error (`ObjectEditError`), not a silent reparent.
- **2026-10-10: empty groups are removed in the same commit (criterion
  14).** One crate-private helper, `remove_emptied_groups(touched_parents)`,
  runs before `commit_with_label` in every command that deletes or replaces
  objects: delete, last-node delete, `replace_with_path`, the Break apart and
  Split commands, and later Cut, Fracture and Flatten. A test lists those
  commands so that a new one cannot skip it.
- **2026-10-10: concurrency.** Two peers who group the same object at the
  same time get Loro's tree-move resolution: one move wins, and the other
  group can end up empty. Readers tolerate an empty group. It draws nothing,
  cannot be hit, and the next local command that touches its parent removes
  it. There is no repair commit on open. A move into a group that a peer
  deleted deletes the node with it (Loro semantics). ADR 0009 §1 accepts
  this, and undo reports it.
- **2026-10-10: reads become tree walks.** Code that iterates
  `tree.roots()` today (render, hit test, bounds, Fit to content, selection,
  validation, the object read cache of 0044) walks the tree depth-first in
  sibling order instead. Paint order is pre-order. A hit on a leaf resolves
  to the context's top-level ancestor (criterion 8). One `document-core`
  function lists the children of a context (`None` = root), and one lists
  the leaf descendants of a node. Every caller uses these two and writes no
  walker of its own.
- **2026-10-10: crate placement.** `curvyo-document-core`: `groups.rs`,
  validation, codec, a deep duplicate that gives new ids to the group, every
  descendant and every anchor (criterion 13), and the format bump.
  `curvyo-ui-core`: context stack, selection and hit resolution, marquee and
  Alt-click candidates per context, Escape cascade step, availability and
  refusals of Group and Ungroup, expansion of a group selection to leaves
  for the 0019 box, the style edit and the Node tool. The group refusal of
  criterion 28 goes into the availability functions of the commands that
  are already merged (Booleans, Combine, Break apart). Commands that merge
  after 0023 add it themselves. `curvyo-render-core`: the 35 % dimming
  outside the context and the context readout. `curvyo-editor-wasm`: the
  context in `Session` and its fallback (criterion 20), `session/group.rs`
  and `wasm_group.rs`, and the Ctrl+G and Ctrl+Shift+G entries in the key
  gate. `frontend`: rail buttons and the readout per the UX notes.
- **2026-10-10: format version and golden files.** The bump takes the next
  free number at merge, with a named constant and a doc paragraph in
  `document.rs`, as for versions 8 and 9. Migration: none. A file from an
  earlier build is a tree of depth 0 and opens unchanged. Golden files: one
  with two nested groups and a mixed leaf set (round trip, criterion 27), one
  33 levels deep (refused), and one where a path node has a child
  (refused). The `specs/README.md` format table names the number in the PR.

## Shared files and order

- 0040 (format version, document root) and 0046 (format version): do not
  build in parallel. Whichever merges second renumbers.
- 0041 `object-history` (object identity across Group and Ungroup) and 0042
  (renderer, which 0023 touches for dimming): rebase-level conflicts only.
- Every command spec that selects objects (0035 to 0038, 0048): same
  `ui-core` availability files.

## Flagged (defaults taken, no customer question)

1. Criterion 27's "cycle" cannot occur in a Loro tree. The tester covers
   depth > 32 only (PO to reword).
2. Concurrently emptied groups survive on disk until the next local command
   touches their parent. This is visible to nobody (they draw nothing).
