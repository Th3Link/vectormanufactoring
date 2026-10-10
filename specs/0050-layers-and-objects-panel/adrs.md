# ADRs for Layers and objects panel

Architect review, 2026-10-10. This replaces the product owner's draft. The spec needs **one new
ADR, [0015](../../docs/adr/0015-node-references-and-ancestor-resolved-state.md)**, which is
`needs-customer` and shared with `0049`. It is not an in-place amendment of ADR 0002 §5, for
the reason given in 0015's preamble: 0002 is accepted, so changing it takes a new ADR. Layers
need no other ADR: a layer is the group node of `0023` with one more register. No new crate, no
new dependency, no trait, no generic.

## Depends on

- [ADR 0015](../../docs/adr/0015-node-references-and-ancestor-resolved-state.md) §2, §4, §6
  (Proposed): the node registers `name`, `visible`, `locked` and `opacity`, the effective values
  resolved through the ancestors, tolerance of merge residue, and the single read pass.
- [ADR 0002](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  - §5: sibling order is z-order, and reparenting is a tree move that keeps the `NodeId`.
  - §8: nodes are assigned to layers. The job assignment of a layer is `0027`.
- [`specs/0023-groups/adrs.md`](../0023-groups/adrs.md): the group node, the two tree functions
  (children of a context, leaf descendants), `remove_emptied_groups` and the depth limit of 32.
- [ADR 0009](../../docs/adr/0009-concurrent-editing-semantics.md):
  - §2: the active layer, the expansion state, the panel target and the scroll position are
    ephemeral.
  - §3: each register is last writer wins.
- [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) §1 and §11: every action is one
  step, and Delete layer is revived by one injected move (decision 8).
- [ADR 0004](../../docs/adr/0004-persistence-and-cross-machine-sync.md) §9: format bump.
- [ADR 0001](../../docs/adr/0001-ui-framework-and-canvas-rendering.md): opacity is multiplied per
  paint. The cost of compositing a group as a whole is in ADR 0015's Consequences.

## Feature-local decisions

1. **2026-10-10: stored shape (format, protect this).**
   - A layer is a `0023` group node (`group: true`) with one more register, `layer: true`, at the
     top level of the objects tree.
   - Any node may carry `name` (string), `visible` (bool), `locked` (bool) and `opacity` (`f64`
     in 0..=1). An absent key reads as no name, visible, unlocked and 1, so files from earlier
     builds open unchanged. The keys do not clash with existing ones (`fill_opacity` and
     `stroke_opacity` are style; `id` and `kind` live in anchor maps).
   - A name is cleaned when it is written: trimmed, cut to 128 characters, control characters
     and bidi controls removed. A name read from a file is cleaned for display only, and the
     file is left as it is (criterion 17).
   - A file is damaged if a register has the wrong type, if `opacity` is not finite or lies
     outside 0..=1, or if `layer` is set on a node that is not a group.
   - A `layer` marker below the top level is read as a plain group (merge residue, ADR 0015 §4).
2. **2026-10-10: `remove_emptied_groups` skips layers.** This amends `0023`'s decision and
   criterion 14: a new layer is empty, and a maker empties a layer on purpose. The test that
   lists the removers gets a layer case.
3. **2026-10-10: the read becomes one pre-order pass of nodes.** The object cache of `0044`
   (`object_cache.rs`, one read per document version) becomes one pre-order list of node reads.
   Each node read holds:
   - its id, parent and depth;
   - its kind (layer, group or leaf with its `ObjectSnapshot`);
   - its own registers;
   - its effective visible, effective locked and effective opacity;
   - the nearest ancestor that hides or locks it, which the inherited-state description of
     criterion 22 needs.

   The walk carries the ancestor values down, so it is O(n) with no per-node ancestor walk.
   Hidden leaves are still resolved, because `0049` clones need hidden origins. The renderer and
   the hit test skip them by flag. A layer toggle is one register write and one re-read. The
   150 ms of criterion 50 is therefore the cost of re-reading 10,000 objects, the same as after
   any other edit today. The slice measures it. If it fails, an incremental cache is a separate
   decision and is not built ahead.
4. **2026-10-10: consumers read the flags; none walks the tree.** Render, hit test, marquee,
   lasso, Alt-click cycle, Ctrl+A, eyedropper, Fit to content, status measures, the Node tool and
   the Pen's continue and connect all read the effective flags from the read. One test lists them
   (criterion 19). `0023`'s "children of a context" function, called for the document top level,
   returns the loose objects and the direct children of every layer in stacking order, without
   hidden or locked nodes. This is the one place that makes layers transparent (criterion 42).
   `ui-core` selection and `hit_test_object.rs` write no walker.
5. **2026-10-10: the selection invariant (criterion 14).** After every change of the document
   version (a local commit, an import or an undo), the session drops from the selection every node
   that is effectively hidden or locked. It does this in the same pass that applies `0023`'s
   context fallback, before the frame is drawn. The cost is O(selection) set lookups in the read.
6. **2026-10-10: no `output_objects()` in this slice.** No exporter and no job generator exists
   yet. A core function whose only callers are tests is the speculative API that `CLAUDE.md` §5
   rules out. The decision "hidden is not output, locked is" is recorded in ADR 0015 §2 and here.
   `0024` and `0029` build their output from the read's effective flags (clones resolved,
   `0049`) and carry the end-to-end test. Flagged below for criterion 24.
7. **2026-10-10: panel rows are a pure window over a cached flattening (criteria 48 to 52).**
   - `layer_rows(read, collapsed, selection)` in `curvyo-ui-core` flattens the visible rows once
     per document version and collapse change, and the session caches the result.
   - `rows_window(first, count)` slices that cache in O(count). Only the window crosses the wasm
     boundary.
   - Each row carries:
     - id, depth, kind, and the name or default name;
     - own and effective flags, with the ancestor that causes an inherited state;
     - opacity;
     - "contains the selection";
     - `aria-posinset` and `aria-setsize`.
   - The total row count comes from the same cache, so the scrollbar is exact.
8. **2026-10-10: moving and deleting nodes.**
   - **Move rules.** One pure function in `curvyo-ui-core`, `plan_move(read, moved, target) ->
     Result<MovePlan, MoveRefusal>`, serves the drag, the Alt+arrow keys and the canvas z-order
     keys. It refuses:
     - a move into the node itself or one of its descendants;
     - a move into a locked container;
     - a move of a locked node;
     - a layer anywhere but the top level;
     - a depth above 32.
   - **The move command.** `Document::move_nodes(plan)` (label `move_nodes`) is one commit of
     tree `mov` / `mov_after` / `mov_before`, which keeps ids, registers and clone links
     (criterion 31). It checks the same rules again before the first write.
   - **Delete layer** (label `delete_layer`) does three things in one commit:
     - It calls `0049`'s `unlink_clones_of_removed` for the layer's subtree, once `0049` exists.
     - It deletes the layer node itself in **one** tree delete. Loro keeps the subtree under the
       deleted node, so the injected move of ADR 0014 §11 revives the layer with every
       descendant and every id (criterion 45).
     - It does **not** delete each descendant on its own. That would turn one undo into N
       revives.
9. **2026-10-10: opacity per paint (criterion 27, Question 5 A).**
   - The render change is in `curvyo-render-core` `artwork.rs`: each paint's alpha is multiplied
     by the effective opacity.
   - The cost of option B is in ADR 0015's Consequences: one more render pass and texture per
     faded container per frame.
10. **2026-10-10: the tree form of `document.json` (decided once for groups and layers).**
    `document.json` is the non-authoritative view of ADR 0004 §1.
    - Each node is written as `{ "id", "kind": "layer" | "group" | <leaf fields as today>,
      "name"?, "visible"?, "locked"?, "opacity"?, "children"? }`.
    - Registers at their defaults are omitted.
    - `0023` did not fix this form, so the first of `0023` and `0050` to merge writes it. `0023`
      leaves out the registers.
11. **2026-10-10: duplicate.** `copy_map` copies every key, so a copy keeps its name and flags
    (the product owner's default: the same name).
12. **2026-10-10: concurrency.**
    - Each register is last writer wins.
    - If one peer hides a layer while another moves an object into it, the object is hidden.
    - If one peer locks a node that another peer has selected, the other peer's next read drops
      the node from its selection.
    - Two peers who move the same node: one move wins (Loro), and no cycle can form.
    - **Question 10:** default A. B can be done reliably. The log identifies the case: the
      node's last move targets a parent whose delete does not include that move in its version
      vector. Lifting the node out needs the injected move of ADR 0014 §11, because a deleted
      node cannot be moved through the handler API. It is deferred to the sharing story
      (ADR 0015, Consequences), together with merged trees deeper than 32 levels.
13. **2026-10-10: crate placement.**
    - `curvyo-document-core`: the codec and validation, the registers, the read pass with
      effective values, `move_nodes`, `new_layer`, `delete_layer`, the setters
      (`set_node_visible`, `set_node_locked`, `set_node_opacity`, `rename_node`; each takes many
      ids and makes one commit), and the format bump.
    - `curvyo-ui-core`: `layer_rows`, `plan_move`, the selection filter and the transparent
      context.
    - `curvyo-render-core`: alpha.
    - `curvyo-editor-wasm`: the row window, the commands and the key gate (Shift+Ctrl+L, Alt+arrows
      and the Proposal keys of criterion 33).
    - `frontend`: the tab.
14. **2026-10-10: format version.**
    - The next free number at merge: 12 if `0023` takes 11.
    - An older build checks the container's `format_version` per file, so it refuses **every**
      file saved by this build, with or without layers. A file from an earlier build opens
      unchanged and is not rewritten on open.
    - Golden files:
      - `layers_v<N>.curvyo`: a layer holding a group, a hidden node, a locked node, opacities,
        names with markup, and a loose object;
      - one damaged file (opacity 1.5);
      - one file with a `layer` marker inside a group, which reads as a plain group.

## Shared files and order

Built after `0023`, and after `0043` for the tab strip. It runs alone among `0023`, `0049` and
`0050`, because all three touch the read pass, the selection context and the panel files. `0044`'s
Ctrl+A and `0015`'s Fit to content are amended in this slice's PR. Whichever of `0020` and this
spec merges second adds the operation names of criterion 44 to `0020`'s table.

## Flagged for the product owner (defaults taken, no customer question)

1. **Criterion 28 contradicts `0049` decisions 2 and 7.** It multiplies in the origin's own node
   `opacity`. ADR 0015 §3 says a clone reads outline and style, and no node register. Remove
   "times the origin's own `opacity`". The clone's alpha is then the origin's paint opacity times
   the clone's own `opacity` times the opacities of the clone's ancestors.
2. **Criterion 24.** Decision 6 builds no `output_objects()` now. The criterion should either be
   tested through the read (the renderer's leaves are exactly the effectively visible ones, and
   a locked leaf is among them) or move to `0024` and `0029`. Default: test it through the read.
3. **Criterion 27, test sentence.** "and so is the stroke at its own 100 %, 25 %" does not parse.
   It should read: "the stroke, at its own 100 %, is painted at 25 %".
4. **Criterion 46, wording.** The earlier build refuses every file saved by this build, not only
   files that use the new registers.
5. **Criterion 47 and `0023` criterion 27.** "Deeper than 32 is damaged" stays for now. Under
   sharing, two legal moves can merge into 33 levels. The sharing story relaxes the reader, which
   needs no change to any file.
