# ADRs for Linked clone

Status: written by the product owner as a list of the ADRs touched and the points the architect must settle. The architect confirms or replaces it before the spec is Ready. Nothing below is a decision yet.

## Is a new ADR needed? Product owner's recommendation: an amendment of ADR 0002 §5, `needs-customer`

This feature adds a node kind to the document model (a clone with an origin id and a matrix), a `format_version` bump, and one exception to "no inheritance" (ADR 0002 §5): a clone reads its style from its origin. `CLAUDE.md` §3 sends every document-model change to the customer.

**Precedent.** The compound path (`0016`) and the group (`0023`) were new node shapes too. The customer accepted each as a question in the spec (`0016` Question 1, `0023` Question 1) and the architect designed it in `adrs.md` with no new ADR number, because ADR 0002 §5 and §6 already allow the tree and the kinds. A clone node fits the same way.

**What does not fit §5 as written** is the "no inheritance" sentence, which exists so that a replicated document has no shared parent register for concurrent edits to contend on. A clone's style read from its origin is one reference to one node (no cascade, no precedence), and `0050` makes visibility, lock and opacity resolve through the ancestors. Recommendation: **no new ADR number; a dated amendment in place to ADR 0002 §5**, as §10 was amended on 2026-10-08, that names these two exceptions and why they do not bring back contention (a style reference is read-only, the flags are single registers). It is `needs-customer`; the customer answers once for this spec and `0050` (the group question is already decided). If the architect judges the amendment too large for a note, a new ADR with the next free number (0015) is the alternative; the content is the same.

## ADRs this feature depends on or extends

- [ADR 0002](../../docs/adr/0002-document-model-units-and-svg-round-trip.md) §5 (tree, ids and sibling order from the CRDT, each node its own affine transform and fully resolved style, no inheritance): extended with a clone node. §6 (paths, primitives, "object to path" explicit): a primitive is converted as part of making a clone, by the same function. §9 (every edit is one commit with a label): the labels are `linked_clone`, `unlink_clone`, `remove_orphan_clones`. §10 (SVG): a clone is expanded in the export (Question 7 of the spec).
- [ADR 0004](../../docs/adr/0004-persistence-and-cross-machine-sync.md) §9 (versioned format, refuse a newer file): a bump, because an older build would read a node without an outline as a damaged or empty path.
- [ADR 0009](../../docs/adr/0009-concurrent-editing-semantics.md) §3 (merge granularity per field): the matrix must be one atomic register; orphans arise from concurrent delete and clone.
- [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) §1 and §11 (restore engine, revive a deleted node): undo of a delete that unlinked clones must change the kind of existing nodes back.
- [ADR 0003](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md): the kernel reads a clone's resolved outline; no change to the kernel.

## Points for the architect

1. **Storage.** Proposal: the clone node's meta map holds `shape = "clone"`, `clone_origin` (the origin's `NodeId` as the id string) and `clone_matrix` (one list value `[a, b, c, d, e, f]`, written whole, like `background_color` in 0040). Not `clone_of`: `0041` uses that key for a history copy. No anchors, no style keys, no `rotation`.
2. **Resolved read.** Proposal: the document read resolves a clone into a path-shaped `ObjectSnapshot` (anchors and handles transformed, the origin's anchor ids reused, the origin's style) that carries `clone: Some { origin, matrix }`. Render, hit test, bounds, Booleans, exporters and the job generator then need no change (`document.json` is the one view that lists a clone as a clone, criterion 40), and a test pins "path read of a clone equals path read after Unlink" (criterion 41). Compare the audit table method of `0016`: list every place that assumes "an object has its own outline", and give each a test.
3. **Resolve cost.** The object cache (`object_cache.rs`, one read per document version) is the place to resolve. Budget in criteria 43 and 44: 10,000 clones of a 100-node path read in 300 ms. If a per-version full resolve is too slow, a per-clone cache keyed by (origin version, matrix) is the fallback; say which.
4. **One exception to "no inheritance".** The clone's style is read from the origin at resolve time (decision 2 of the spec). Confirm that this is not a cascade in the sense of ADR 0002 §5 (it is one reference to one node, no precedence rules), or propose a different shape.
5. **Orphans and repair.** Clone with a missing, non-path or clone origin is an orphan; Open and merge run one repair commit that deletes them (precedent: ADR 0012's repair commit when an import leaves no page). Check: two peers both repairing is harmless (a tree delete of a deleted node); the repair is a normal step in the history list.
6. **Bake on removal (criterion 31).** Every command that removes an origin must unlink its clones in the same commit. Proposal: one function in `document-core` that every remover (`delete_objects`, `replace_with_path`, Break apart, the Node tool's last-node delete) calls, so a new remover cannot forget it; a test lists all removers.
7. **Undo of a bake (criterion 36).** The step changes the kind of a live node (clone to path, with anchors). The restore engine writes old values field by field; check that "kind change plus created anchors" restores, with the precedent of the 0019 / 0047 conversion in one commit.
8. **Transform maths in `ui-core`.** The clone's gesture result (G·M, or G·M·G⁻¹ with its origin in the set, or unlink first when G is singular) is a pure function. Put it with the group transform of `0019`, not in the session.
9. **Format version.** Next free at merge (`specs/README.md`). Golden file `clone_v<N>.curvyo`: one origin, two clones, one compound-path origin, one rectangle converted by making its clone.
10. **Build order.** This spec does not need `0023` or `0050` to build (it needs only the ADR). It collides with `0050` in the document read model and the Style/Layers panel files, and with `0023` in the selection context, so it does not run beside either. Recommended order: `0023`, `0050`, then this one, so the rows of `0050` show the link glyph from the first day; the lead may reverse it.

## Feature-local decisions

- 2026-10-10: Proposed by the product owner, not yet architect-confirmed: the names `clone_origin` and `clone_matrix`; no chains (flatten at creation); no style overrides; bake on removal; orphans removed by a repair commit; output expands.
