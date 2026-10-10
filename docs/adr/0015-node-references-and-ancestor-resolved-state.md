# ADR 0015: Node references and ancestor-resolved node state

**Status:** Proposed (2026-10-10), `needs-customer` (document model, `CLAUDE.md` §3).
If accepted, it supersedes one sentence of
[ADR 0002](0002-document-model-units-and-svg-round-trip.md) §5 ("Each node carries its own
affine transform and its own fully resolved style: no CSS cascade, no inheritance"), and 0002's
Status line points here. Asked by `specs/0049-linked-clone` and
`specs/0050-layers-and-objects-panel`.

## Context

ADR 0002 §5 gives every node its own resolved style and rules out inheritance, so that a
replicated document has no shared parent register for concurrent edits to contend on. Groups
(`0023`, accepted) are interior nodes without style or transform, which fits that rule.

Two specs now need a node whose meaning depends on registers that another node owns:

- `0050`: layers and groups get `visible`, `locked` and `opacity`. A hidden layer hides
  everything in it, and a hidden node is not output.
- `0049`: a linked clone shows another path's outline and style through its own matrix.

The product owner proposed an in-place amendment of 0002 §5. It is a new ADR instead because
0002 is accepted and the index rule says "every further change to any of these decisions is a new
ADR" (ADR 0014 superseding one sentence of 0004 §2 is the precedent). The 2026-10-08 change to
0002 §10 only removed an entry. This one adds semantics with merge consequences.

### Options: flags and opacity of layers and groups

- **A. Resolve through the ancestors when the document is read (chosen).** Each container has
  one register per flag, and a node's effective state combines its own register with every
  ancestor's.
- **B. Copy down.** A toggle on a layer writes the flag into every descendant. Rejected. One click
  on a layer of 10,000 would write 10,000 registers and make one 10,000-object history step
  (`0050` criterion 50 asks for one write). An object a peer moves into the layer while the layer
  is being hidden stays visible, so the copies go stale under merge. "Show the layer and every
  child is as it was" (`0050` criterion 22) would need a second register per child anyway.
- **C. Per-user view state.** Rejected. Output depends on it (hidden is not cut, `0050`
  decision 5), so it belongs in the document.

### Options: linked clone

- **A. Reference (chosen).** The clone stores the origin's id and a matrix. The outline and the
  style are read from the origin when the document is read.
- **B. Copy on write.** The clone stores a full path, and every edit of the origin rewrites all
  of its clones in the same commit. Rejected. One node drag on an origin with 10,000 clones of
  100 nodes would write a million anchors to the log. Worse, a peer's concurrent edit of the
  origin reaches only the clones that peer knows about, so after a merge the clones silently
  disagree with the origin, which is the failure the link exists to prevent.
- **C. SVG `<use>` with per-property overrides (Inkscape).** Rejected for now. It is a cascade
  with precedence rules, exactly what §5 excluded, and `0049` decision 2 needs no overrides.

## Decision

1. **The superseded sentence now reads:** "Each node carries its own fully resolved style; there
   is no style cascade. A node's meaning may depend on another node in exactly the two ways of
   ADR 0015 §2 and §3." A third way needs a new ADR. Group and layer nodes carry no style
   (`0023`).
2. **Ancestor-resolved node state.** Any node may carry four node registers, which are not style:
   `name` (string, absent = none), `visible` (bool, absent = true), `locked` (bool, absent =
   false) and `opacity` (number in 0..=1, absent = 1). The effective values are:
   - visible: the node's own flag and every ancestor's must be on;
   - locked: the node's own flag or any ancestor's is on;
   - opacity: the product of the node's value and its ancestors' values along the chain.

   `name` is never inherited. Nothing is copied into children, and a child keeps its own
   registers, so showing an ancestor again restores the child exactly as it was.
3. **Clone reference.** A clone node (`shape = "clone"`) stores two registers:
   - `clone_origin`: the origin's `NodeId` string;
   - `clone_matrix`: one list value of six `f64`, written whole. It is a value, not a container,
     so a merge keeps one matrix and never mixes entries.

   A clone reads exactly two things from its origin: the outline(s) and the `Style`. It reads none
   of the node registers of §2, so a hidden, locked or faded origin leaves its clones unchanged.
   A reference is one hop: the origin must be a live path or compound path and never a clone.
   Anything else makes the clone an **orphan**.
4. **Readers tolerate merge residue.** Concurrent edits can leave a document that no single peer
   wrote, and none of these states counts as damaged:
   - an orphan: it is not drawn, not hit and not output, and the repair of §5 handles it;
   - `clone_*` keys on a node whose `shape` is not `clone`: ignored;
   - other keys on a clone node: ignored;
   - a `layer` marker below the top level: the node is read as a plain group.

   "Damaged" stays for values that no writer of ours produces: a matrix that is not six finite
   numbers, an opacity outside 0..=1, or a flag that is not a boolean.
5. **Removal and repair.** A command that removes an origin, either directly or as part of a
   removed subtree, unlinks the origin's surviving clones in the same commit. They become paths
   that look the same. After Open and after every merge, the editor runs one repair commit that
   removes orphans. The set of orphans is a pure function of the converged state, so two peers
   that repair at once remove the same nodes and converge.
6. **One read.** One pass over the tree per document version resolves the effective state and
   every clone. Each origin is read once per pass, not once per clone. Consumers (render, hit
   test, selection, panels, output) use the resolved values. None of them walks ancestors or
   follows a reference itself.
7. **Format.** Each of the two specs bumps `format_version` at merge (ADR 0004 §9). An older build
   refuses every file saved by the newer build, whether or not it uses the new registers.

## Consequences

- Every register still has one owner, so concurrent edits combine without conflict. If one peer
  hides a layer while another moves an object into it, the object ends up hidden. If one peer
  edits an origin while another transforms its clone, both changes apply.
- The read pass costs O(nodes + clone anchors) per document version. That is already the cost of
  the object cache of `0044`. A layer toggle is one write and one re-read.
- A clone's look depends on a node that may sit elsewhere, hidden or locked. The panel shows the
  link (`0049` criteria 22 and 23).
- Undo of an unlink, or of a removal that unlinked clones, changes a node's kind back field by
  field (ADR 0014 §1). The engine must treat an absent key as a value: it deletes the anchors and
  style keys the step created and restores `shape`, `clone_origin` and `clone_matrix`. The
  primitive-to-path conversion of `0047` is the same case.
- Opacity is multiplied per paint (`0050` Question 5, A). Compositing a group or layer as a whole
  (B) would need, per faded container and per frame, one more render pass into a viewport-sized
  texture in the wgpu/WebGL2 renderer (ADR 0001). That is affordable for a few layers and not
  for many faded leaves. It would be a separate decision.
- **Left for the sharing story.** None of these can happen until two peers edit one document, and
  each fix only relaxes a reader or adds a repair, so no file written today has to change:
  - A node that one peer moves into a group or layer while another peer deletes that container
    is deleted with it (Loro tree semantics, accepted for groups in `0023`). The log can tell
    this case apart: the delete's version vector does not contain the move. The node can then be
    lifted out with the injected move of ADR 0014 §11.
  - The orphan repair deletes a clone that one peer made while another peer deleted its origin.
    `0049` criterion 31 would rather keep it as a path. The origin's tombstone still holds its
    outline (ADR 0014 §11 relies on Loro keeping the containers), so the repair can unlink the
    clone instead.
  - Two moves, each legal on its own peer, can merge into a tree deeper than 32 levels, which the
    `0023` reader refuses as damaged. Walkers are iterative, so the reader can accept any depth.
- Named styles or style overrides on a clone need a new ADR. This one does not allow them.

## Verification

Tests in `curvyo-document-core`:

- effective state over a three-level chain;
- a clone resolves to the same path as after Unlink, within 1e-9 mm, with an equal style;
- two replicas repair the same orphans concurrently, merge, and have equal state;
- every residue of §4 reads without an error;
- 10,000 clones of a 100-node path read within the budget of `0049` criterion 43.

## Question for the customer

1. **Accept §1 to §7?** *A (default, recommended):* yes, which makes `0049` and `0050` possible
   as specified. *B:* no. `0050` then keeps names and the tree but has no flags or opacity, which
   defeats R-EDIT-009, and `0049` is dropped.
