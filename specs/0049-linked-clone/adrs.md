# ADRs for Linked clone

Architect review 2026-10-10. This replaces the product owner's draft. The spec needs **one new
ADR, [0015](../../docs/adr/0015-node-references-and-ancestor-resolved-state.md)**, which is
`needs-customer` and shared with `0050`. It is not an in-place amendment of ADR 0002 §5: 0002 is
accepted, and the index rule sends every change of an accepted decision to a new ADR (ADR 0014
superseding one sentence of 0004 §2 is the precedent). The content is what the product owner
proposed: one reference from a clone to its origin, which is not a cascade. No new crate, no new
dependency, no trait, no generic.

## Depends on

- [ADR 0015](../../docs/adr/0015-node-references-and-ancestor-resolved-state.md) §3 to §6
  (Proposed): the clone reference, tolerance of merge residue, unlinking on removal, the orphan
  repair and the single read pass. It supersedes the "no inheritance" sentence of ADR 0002 §5.
- [ADR 0002](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  - §5: ids, and reparenting as a tree move. The link is by id, so moves never break it.
  - §6: "object to path" is explicit; criterion 2 calls the existing conversion.
  - §9: one commit per command, with the labels `linked_clone`, `unlink_clone` and
    `remove_orphan_clones`.
  - §10: SVG writes a clone as a path (Question 7, A).
- [ADR 0009](../../docs/adr/0009-concurrent-editing-semantics.md) §3: last writer wins per
  register. The matrix is one register.
- [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) §1 and §11: undo restores a kind
  change field by field and revives a deleted origin with its id (see decision 9).
- [ADR 0004](../../docs/adr/0004-persistence-and-cross-machine-sync.md) §9: format bump, and a
  newer file is refused.
- [ADR 0003](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md): no change. The
  kernel receives the resolved path.

## Feature-local decisions

1. **2026-10-10: stored shape (format, protect this).** A clone is a leaf of the objects tree. Its
   meta map holds:
   - `shape = "clone"`;
   - `clone_origin`: the origin's `NodeId` in the text form `0041` uses;
   - `clone_matrix`: one `LoroValue::List` of six `f64`, `[a, b, c, d, e, f]`. It is written with
     `insert` as a whole value and never with `insert_container`, so that a merge keeps one matrix
     (criterion 13).

   A clone has no anchors, `closed`, `extra_subpaths`, style keys or `rotation`. The keys do not
   clash: `0041`'s `clone_of` is a history key.

   Validation runs only when `shape == "clone"`. The file is damaged if `clone_origin` is missing
   or not an id string, if `clone_matrix` is missing, is not six numbers, or has an entry that is
   not finite or lies beyond ±1e7, or if the clone node has children. Other keys on a clone node
   and `clone_*` keys on any other node are merge residue: they are ignored, not damaged
   (ADR 0015 §4). That includes an `opacity` key on a clone: a clone has none (criterion 49), and
   `set_node_opacity` of `0050` skips clones.
2. **2026-10-10: the read is a new variant, not a path with an optional field.** The read is
   `ObjectSnapshot::Clone(CloneSnapshot { id, origin, matrix, resolved: PathSnapshot })`.
   `resolved` holds:
   - the clone's `NodeId`;
   - the origin's anchors transformed (each point `p → M·p`, each handle `v → L·v`), keeping the
     origin's anchor ids;
   - the origin's `closed` state and extra outlines;
   - the origin's `Style`;
   - `rotation` 0.

   The read of `0050` gives the clone an effective opacity of the origin's `opacity` register
   times the `opacity` of the clone's own ancestors (ADR 0015 §3, criterion 49). `visible`,
   `locked` and `name` are the clone's own registers.

   Readers that draw, hit, measure or feed the kernel reach `resolved` through the arm they
   already have for paths.

   Rejected: the product owner's `Path` with `clone: Option<…>`. Every writer that takes a path
   snapshot back and writes anchors would then silently write anchors into a clone node:
   `transform_objects` (`Planned::Path`), `resize_path`, `rotate_object`, node edits, Join and
   Split. A new variant makes the compiler list every match site, which is the lesson of the
   technical-debt entry "One-outline assumptions are found by audit, not by the compiler". The
   writers get `Planned::Clone`, which writes only `clone_matrix`. Node edits, Join, Split,
   segment bending, the Pen's continue and connect, and Object to path refuse a clone with the
   refusal pattern of `0016` and `0023` (a code plus the offenders).
3. **2026-10-10: resolving, and its cost (criteria 43 and 44).** Clones are resolved in the one
   read pass that the object cache of `0044` already makes once per document version. Each origin
   is read from Loro once into a map local to the pass, and every clone transforms that snapshot
   in memory. There is no cross-version cache per clone: 10,000 clones read less from Loro than
   10,000 copies do, and they hold the same memory (about 70 MB at 100 nodes each). Origins are
   resolved whatever their effective flags, because a hidden origin still feeds its clones
   (`0050` criterion 25). The origin's `opacity` register is read in the same origin read, so
   criterion 49 adds no cost; a change of it is a new document version and a re-read like any
   other edit. `Document::object(id)` on one clone reads its origin as well. If
   criterion 43 fails, the next step is to share the origin's outline and transform it at
   tessellation. That would be measured first, not built ahead.
4. **2026-10-10: one matrix type and one rule for transforming a set.** `curvyo-document-core`
   `units.rs` gets `Affine`, the first matrix type of the workspace: compose, `inverse() ->
   Option` (`None` when |det| < 1e-9), and apply to a point and to a vector. One pure function,
   `clone_matrix_after(m, g, origin_moves) -> Affine`, returns G·M, or G·M·G⁻¹ when the origin
   is transformed in the same command. Two kinds of caller use it:
   - `curvyo-ui-core`, for the gesture preview and its result (criteria 15 and 16);
   - every `curvyo-document-core` writer that moves a set: `translate_objects`,
     `transform_objects`, `Document::resize`, `fit_to_content`, and the group transform of
     `0023`.

   Reason: Fit to content moves origins and clones together without `ui-core`. A clone given
   T·M there would also follow its origin's move by L·t and land in the wrong place. The
   product owner's point 8 placed the rule in `ui-core` only, which would miss these writers.
5. **2026-10-10: unlink, and unlink on removal (criteria 28 and 31).** Module `clones.rs` in
   `curvyo-document-core`.
   - `unlink_clones(ids, minted_anchor_ids)` (label `unlink_clone`) writes, under the same id and
     tree position:
     - the resolved anchors with fresh ids;
     - `closed` and `extra_subpaths`;
     - a copy of the origin's style;
     - a copy of the origin's `opacity` register once `0050` exists, so the path looks the same
       (criterion 49; flagged for criterion 28 below);
     - and it removes `shape`, `clone_origin` and `clone_matrix`.

     This is the `Convert` case of `transform_objects` (`0047`) in another direction.
   - One crate-private helper, `unlink_clones_of_removed(removed_roots)`, runs before the removal
     in the same commit. It is called by every command that also calls `0023`'s
     `remove_emptied_groups`: delete, last-node delete, `replace_with_path`, Break apart, Split,
     Fracture, Flatten, and `0050`'s Delete layer. It looks for origins among **all descendants**
     of each removed node, because a removed group or layer takes its origins with it. Clones
     inside the removed subtree go with it and are not unlinked. One test lists the removers for
     both helpers.
6. **2026-10-10: the orphan repair (criteria 35 and 39).** `Document::remove_orphan_clones() ->
   usize` (label `remove_orphan_clones`) deletes every clone whose origin is not a live,
   non-clone path or compound path. It checks one hop, so a self-reference or a ring of 10,000
   ends without a loop or recursion. It iterates in tree order and depends on nothing but the
   state, so two peers that repair concurrently delete the same nodes; deleting a node that is
   already deleted does nothing. The session calls it after Open and after `import_updates`, and
   it is an ordinary history step. The function returns the count; the session shows the
   criterion 35 notice ("Removed 2 clones whose original was not found.", 8 s) only when the
   count is above 0. It deletes and never unlinks, as criterion 35 says: an orphan has no outline
   to keep. Today the only reachable orphans come from foreign or hand-edited files. In the merge
   case of the sharing story the origin's tombstone still holds an outline, so unlinking from it
   instead is an option to decide then (ADR 0015, Consequences).
7. **2026-10-10: merge cases.** These are the cases the tests should cover, in the
   `acceptance_0005_peers.rs` style:
   - *Peer A deletes the origin (and unlinks clone C), while peer B transforms C.* After the
     merge, C is a path with A's outline, because only A wrote `shape`. B's matrix is lost, and
     its key may survive on the path as ignored residue. If A then undoes, C becomes a clone
     again. If B's matrix won the race on that key, it is kept, because the engine restores a
     field only where the current value equals A's (ADR 0014 §1).
   - *Peer A deletes the origin, while peer B makes a new clone of it.* The new clone is an
     orphan and is removed by the repair (decision 6).
   - *Two peers unlink the same clone.* The `anchors` key keeps one of the two containers. Both
     have the same geometry and different anchor ids, and the replicas converge.
   - *Two peers transform the same clone.* One whole matrix wins (criterion 13).
   - *Peer A edits the origin, while peer B transforms the clone.* Both edits apply (criterion
     13).
8. **2026-10-10: copying sets (criteria 19, 47, 48).** `copy_map` already copies every key, so
   the copy of a clone starts as a clone of the same origin with the same matrix (criteria 19
   and 48), and the copy of an origin is a path that no clone refers to. Criterion 47 needs one
   more step. One crate-private helper, `relink_copied_clones(copies)`, gets the map from each
   source id to its copy's id that the copy command builds anyway. For every copied clone whose
   source origin is also in the map, it sets `clone_origin` to the origin's copy. For a Ctrl-copy
   move by G, such a clone gets `clone_matrix_after(m, g, true)` (G·M·G⁻¹, decision 4), and any
   other copied clone gets G·M. It runs in the same commit as the copy. Its callers are
   `duplicate_objects`, `0023`'s deep duplicate, and later duplicate layer and paste within a
   project, so no copy path can skip it. A clone outside the copied set is never touched. One
   test per criterion 47 and 48 case, including the Ctrl-copy matrix.
9. **2026-10-10: undo (criterion 36).** No engine change if ADR 0014 §1 already treats an absent
   key as a value: it deletes keys and containers that the step created and restores the
   removed ones. The `0047` conversion is the same case. This slice adds two tests in the `0020`
   style:
   - undo of Unlink gives back a clone with the same origin and matrix, and no anchors key;
   - undo of the Delete of an origin with two clones revives the origin with its id (§11) and
     turns both clones back.

   If the engine does not handle absent keys yet, the fix belongs to `0020`'s restore code, not
   to this slice.
10. **2026-10-10: crate placement.**
    - `curvyo-document-core`: the codec (`shape_codec` gets the `clone` tag), validation,
      `clones.rs`, `Affine`, the new read variant, `Planned::Clone`, the refusals, the
      `document.json` entry (`"shape": "clone"`, `"origin"`, `"matrix"`) and the format bump.
    - `curvyo-ui-core`: availability, the gesture result, the panel lines, and Select original /
      Select clones as view state.
    - `curvyo-render-core`: the new variant's arm only.
    - `curvyo-editor-wasm`: the repair call after Open and after an import, and the commands.
    - `frontend`: the buttons and notices.
11. **2026-10-10: format version.** The next free number at merge: 13 if `0023` (11) and `0050`
    (12) merge first. An older build checks the container's `format_version` per file, so it
    refuses **every** file saved by this build, with or without clones. A file from an older
    build opens unchanged. Golden files:
    - `clone_v<N>.curvyo`: one origin, two clones, a compound-path origin, and a rectangle
      converted by making its clone;
    - one damaged file (a matrix of five entries);
    - one file with an orphan.

    The 100,000-clone and 10,000-ring cases are built in the tests and not committed.

## Shared files and order

Build after `0023` and `0050`, as the product owner recommends. All three touch the object read
(`object_cache.rs` and the read pass). This spec and `0050` also share the panel files, and this
spec and `0023` share the selection context. It runs alone among those three. The removers'
helper is shared with `0023`'s `remove_emptied_groups`.

## Flagged for the product owner

Resolved in the spec fixes of 2026-10-10: orphans (criterion 35), copying sets (criteria 47 and
48), and the clone's opacity (criterion 49 and `0050` criterion 28). Two points remain, no
customer question:

1. **Criterion 28 (with criterion 49).** Unlink copies the style but says nothing about
   opacity. Without a copy of the origin's `opacity`, an unlinked clone of a faded origin jumps
   to 100 % and "the drawn outline before and after is the same" fails in look. Default (decision
   5): copy it. The criterion should say so; criterion 31's unlink on removal follows.
2. **Criterion 37.** "A file with a clone opened by an earlier build is refused" is true but too
   narrow: the earlier build refuses every file saved by this build.
