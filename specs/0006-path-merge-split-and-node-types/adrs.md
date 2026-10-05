# ADRs for "Path merge/split and a third node type"

This slice changes one stored value: an anchor's `kind` gains a third
variant and its existing second variant is renamed. That is the document
model and a file format we write, so the wire tags and the reading rule are
written out below. Join and Split are anchor-list surgery on the existing
schema: no new register, no curve evaluation.
**No new crate, no new external dependency, no new `vecmanf-geometry-core`
function, no ADR amendment. `format_version` goes to 5.**

**Build order:** this slice touches `vecmanf-document-core`,
`vecmanf-ui-core`, `vecmanf-render-core` and `vecmanf-editor-wasm`, the same
crates as `object-transform`. It starts after that branch merges
(`CLAUDE.md` §4), and it relies on that slice's `rotation` register (Split
copies it, Join keeps the survivor's).

## Depends on

- [ADR 0002 §5, §6](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  a path is one tree node with a CRDT-minted `NodeId`; an open/closed flag;
  ids are never array offsets. Split's new object gets its `NodeId` from the
  same `tree.create` call `create_path` already uses.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one interaction is one commit. Join, Split, one kind conversion over a
  multi-path selection, and one multi-path node drag are each one commit.
  Every command resolves all ids before its first write; a refusal writes
  nothing.
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md):
  the node selection and the Node-tool editing set are ephemeral and hold
  ids resolved lazily. `kind` stays one LWW register.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  forces the `format_version` bump below.
- [ADR 0003 §1](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md)
  and [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md):
  slice 2's dividing question ("does this evaluate a curve?") answers "no"
  for every criterion here, so nothing reaches the kernel.
- [`specs/0002-path-node-editing/adrs.md`](../0002-path-node-editing/adrs.md):
  the anchor schema (relative handles, the zero vector as a retracted handle,
  `kind` stored not derived), caller-minted `AnchorId`s via
  `AnchorIdMinter`, "a press and release with no movement writes nothing".
- [`specs/0003-primitive-shapes/adrs.md`](../0003-primitive-shapes/adrs.md):
  an edit that transforms an object keeps its `NodeId`.
- [`specs/0004-canvas-navigation-and-selection/adrs.md`](../0004-canvas-navigation-and-selection/adrs.md):
  `ObjectSelection` is the input to AC 6.
- [`specs/0005-object-transform/adrs.md`](../0005-object-transform/adrs.md):
  `rotation` on every object, and `format_version` 4.

## Deliberately not in scope for this slice

- **Undo** (`undo-redo`). Every command here ends in one labelled commit, so
  the undo slice needs nothing more from this one.
- **SVG mapping** (`svg-import-export`). For that slice: Inkscape's
  `sodipodi:nodetypes` maps `c` → Corner, `s` → Asymmetric, `z` →
  Symmetric. Inkscape's `a` (auto-smooth) has no counterpart and imports as
  Asymmetric.
- **Rounded-rectangle outline kinds.** `primitive_outline` writes Corner at
  the line-to-arc joins because no tangent-continuous unequal-length kind
  existed. Those joins are Asymmetric in substance, but changing what
  "object to path" produces is a slice-3 behaviour change that no criterion
  here asks for. Left as it is.

## Feature-local decisions

- **2026-10-05: `AnchorKind` has three variants, and the stored tag of the
  renamed one changes.** The Rust rename `Smooth` → `Symmetric` is
  mechanical (every call site matches on the variant, none on its name).
  The stored string is the real decision, because `kind` is written as a
  string tag in `document.loro` (`path_codec`: `"corner" | "smooth"`) and
  serde writes the variant name in lowercase into `document.json`.

  - **(A) Keep writing `"smooth"` for Symmetric forever; add
    `"asymmetric"`.** No compatibility code. Rejected: the file format we
    write would say "smooth" for the equal-length kind permanently, which is
    the opposite of Inkscape's meaning and of the name this specification
    forbids reusing. `document.json` exists for scripted reading
    (ADR 0004 §1), so the confusion would land on exactly its readers.
  - **(B) Write `"symmetric"` and `"asymmetric"`; read `"smooth"` as
    Symmetric, permanently.** Chosen. One extra match arm in `read_kind`.
    Old anchors are never rewritten, so a file can hold both tags for one
    meaning; that is invisible to every reader that goes through `read_kind`,
    and `document.json` is regenerated from the snapshot on every save, so it
    only ever shows `"symmetric"`. The existing golden fixture
    `paths_v2.vmf` stores `"smooth"` and already asserts the kind; it
    becomes this migration's test with the variant renamed.
  - **(C) Rewrite every `"smooth"` to `"symmetric"` on open.** Rejected.
    Opening a file would write operations: a dirty document nobody edited,
    an undo step nobody made, and an LWW write that can win against a
    collaborator's real conversion of the same node (slice 2's "a click must
    not win against a concurrent real edit").

  ```text
  anchor.kind : "corner" | "symmetric" | "asymmetric"   LWW register
                read also: "smooth" → Symmetric (format_version ≤ 4)
                any other value → Corner (slice 2's lenient read, unchanged)
  ```

  The wasm binding string (`"smooth"` in `wasm_api.rs`, the frontend's
  `"corner" | "smooth"` union, `can_convert_to_smooth`) is not persisted and
  is renamed outright. `DEFAULT_SMOOTH_HANDLE_LENGTH_MM` becomes
  `DEFAULT_HANDLE_LENGTH_MM`.

- **2026-10-05: Asymmetric adds no register.** It uses the same `point`,
  `handle_in`, `handle_out` as the other kinds. Only the rules differ, and
  all of them are elementary arithmetic on `Vec2` in `document-core`:

  | Rule | Corner | Symmetric | Asymmetric |
  |---|---|---|---|
  | drag one handle to `v` (AC 3) | opposite unchanged | opposite = −v | opposite = −unit(v) · \|opposite\| |

  Edge cases of the Asymmetric rule: `v` = zero leaves the opposite handle
  unchanged (no direction to follow); an opposite handle of length zero
  stays zero. The rule is one pure function in `document-core`, called by
  `Document::set_handle` and by any live preview in `ui-core`, so preview
  and commit cannot disagree.

  Conversions (`convert_anchor_kind`, AC 2, 4, 5), per anchor:

  | from \ to | Corner | Symmetric | Asymmetric |
  |---|---|---|---|
  | Corner | no write | default length, tangent (slice 2) | tangent; each side keeps its length if non-zero, else default |
  | Symmetric | kind only | no write | kind only |
  | Asymmetric | kind only | default length, tangent | no write |

  "Tangent" is slice 2's existing `neighbour_tangent`. **Converting a node
  to the kind it already has writes nothing.** Today `Make smooth` on a
  smooth node resets its handles to the default length, while
  `NodeToolbarState`'s own doc comment calls that conversion "a harmless
  no-op"; slice 2's AC 11 does not cover the case. With Asymmetric the
  difference becomes visible (re-applying AC 2 would rotate the handles to
  the chord tangent), so the documented intent wins. This also keeps the
  slice-2 rule that an unchanged rewrite must not race a real edit. A mixed
  selection converts only the anchors whose kind differs, in one commit; if
  none differ, there is no commit.

- **2026-10-05: one Node-tool session over several paths (AC 6, 7).**
  `NodeSelection` today holds one `path: Option<NodeId>` and a list of
  `AnchorId`s. It becomes an **ordered list of `(NodeId, AnchorId)`**, and
  the Node tool holds an editing set of `NodeId`s taken from
  `ObjectSelection` when the tool is entered (primitives in that selection
  are skipped; they have no nodes until "object to path"). Both resolve
  lazily. Order is kept because Join needs "first selected". A segment
  selection stays inside one path. Commands that take a node selection
  (`move_anchors`, `convert_anchor_kind`) take `(NodeId, AnchorId)` pairs
  across paths and still commit once; a multi-path node drag is one commit.
  No new selection type is invented.

- **2026-10-05: Join is `Document::join_endpoints` in `document-core`.**
  Input: the two selected endpoints in selection order, `a` then `b`. The
  midpoint is arithmetic on `Point`, computed inside the command.
  - **Refuses** (writes nothing) unless both ids resolve, both are the first
    or last anchor of an open path, and they are different anchors. For two
    ends of one path, it also refuses when the path has only 2 anchors: the
    result would be a closed 1-anchor path, which `render-core` does not
    draw (`stroke.rs` needs 2 anchors).
  - **Two objects (AC 9).** The path of `a` survives: it keeps its
    `NodeId`, z-position and every non-anchor register (`stroke`,
    `stroke_width`, `rotation`, later the style keys). The path of `b` is
    deleted. If `a` is its path's last anchor, `b`'s path is appended after
    it; if first, prepended. `b`'s path is **reversed** when needed to meet
    at the junction (when `b` is the same kind of end as `a`): list order
    reversed and each anchor's `handle_in`/`handle_out` swapped, which leaves
    the geometry unchanged. The merged anchor keeps `a`'s `AnchorId`; `b`'s
    endpoint is dropped. Its point is the midpoint; the handle facing `a`'s
    side is `a`'s inward handle, the handle facing `b`'s side is `b`'s
    inward handle (after any reversal); the two outward, dangling handles
    are discarded; `kind` is Corner. Moved anchors keep their `AnchorId`s
    (globally unique), so selection survives.
  - **One path (AC 10).** The first anchor survives at list position 0
    with its id; the last is removed; `closed` = true. Same handle and kind
    rule.
  - Moving `b`'s anchors into another movable list is delete plus insert
    (Loro's `mov` works within one list only). A collaborator's concurrent
    edit to one of those anchors is lost with the deleted object; that is
    ADR 0009 §3's delete floor, the same as deleting the object.
  - Returns the merged `(NodeId, AnchorId)` so `ui-core` selects only it
    (AC 11).

- **2026-10-05: Split is `Document::split_at_anchor` in `document-core`,
  Join's inverse.** That framing is accurate topologically: open Split
  undoes a two-object Join, closed Split undoes a one-path Join. It is not
  an exact geometric inverse: Join moves to the midpoint and drops the
  dangling handles, so Split after Join restores the topology and the
  inward handles, and both copies become Corner.
  - The caller passes one fresh `AnchorId` from the existing
    `AnchorIdMinter` for the second copy; no new id mechanism. The first
    copy (incoming handle kept, outgoing zero) keeps the original `AnchorId`;
    the second copy (outgoing kept, incoming zero) gets the new one. Both
    are Corner.
  - **Refuses** for the first or last anchor of an open path (AC 12).
  - **Open path (AC 13).** The original `NodeId` keeps the first part. The
    second part is a **new object** from the same `tree.create` call as
    `create_path`, placed directly above the original in z-order, written
    from the original's `PathSnapshot` with its anchors replaced, so it
    copies every register the snapshot carries (`stroke`, `stroke_width`,
    `rotation`, later the style). Rejected: copying the meta map key by key,
    which would also copy stray keys slice 3 tolerates on paths and needs
    special handling for container values. Anchors after the split point
    keep their `AnchorId`s.
  - **Closed path (AC 14).** Same `NodeId`, `closed` = false, one new
    anchor. The list is rotated with movable-list `mov` operations so each
    existing anchor keeps its element and a concurrent edit to it survives.
    Resulting order: second copy, the anchors after the split point around
    the loop, first copy.
  - Returns both copies' `(NodeId, AnchorId)` for AC 15's selection.

- **2026-10-05: `format_version` goes to 5.** Migration from version 4 is
  the `"smooth"` read alias above and nothing else. The bump is needed for
  the reader: a version-4 reader maps any unknown `kind` string to Corner,
  so it would open a file with Asymmetric nodes and silently turn them into
  corners. That is the silent partial read ADR 0004 §9 forbids.
  `document.json` writes `"corner" | "symmetric" | "asymmetric"`.
  **`stroke-and-fill-styling` moves to 6** (dated note in its `adrs.md`),
  because this slice is now sequenced before it.

- **2026-10-05 (implementer): taken as `format_version` 4, not 5, on
  `main` as actually branched.** The note above assumed `object-
  transform` (slice 5, claiming 4) had already merged. This slice was
  instead built in parallel with `canvas-navigation-and-selection`
  (slice 4) directly off `main`, before either slice 4 or slice 5 had
  merged — `main`'s `CURRENT_FORMAT_VERSION` was still 3 at the time,
  with no `rotation` register at all. Taking 4 (the next free version on
  the branch this was actually built from) rather than pre-claiming 5
  for a predecessor that does not exist here. Consequence: Join and
  Split do not copy or carry a `rotation` field today, because none
  exists yet to copy — whichever of this slice and `object-transform`
  merges second needs its `format_version` (and, for `object-
  transform`, Join/Split's handling of `rotation`) reconciled against
  the other. Flagged as expected integration work in both slices' PRs,
  not a defect in either one considered alone.

- **2026-10-05: the crate boundary.**
  - `vecmanf-document-core`: the three-variant `AnchorKind`, the codec tags
    and read alias, the per-kind handle rule, the conversion table,
    `join_endpoints`, `split_at_anchor`, multi-path `move_anchors` and
    `convert_anchor_kind`, `CURRENT_FORMAT_VERSION = 5`.
  - `vecmanf-ui-core`: the multi-path `NodeSelection` and editing set,
    toolbar state (`can_convert_to_symmetric`, `can_convert_to_asymmetric`,
    `can_join`, `can_split`, computed from the selection with the same
    conditions the commands refuse on), minting the Split copy's id,
    selection after Join and Split.
  - `vecmanf-render-core`: node decorations for several paths, and an
    Asymmetric glyph (the `ux-engineer` chooses it).
  - `vecmanf-editor-wasm` / `frontend/`: binding strings, the new actions,
    the labels.
  - `vecmanf-geometry-core`: no change.

## Flagged to the lead

1. **`format_version` collision, resolved by sequence.** `stroke-and-fill-styling`'s
   `adrs.md` claimed 5. This slice ships first and takes 5; styling goes to 6
   (dated note added there). If the order changes again, swap.
2. **AC 9's traversal sentence holds for one of four endpoint combinations.**
   "The first path's nodes in their original order, then the second's"
   is only true when `a` is the first path's last anchor and `b` the second
   path's first. Start-to-start and end-to-end need one path reversed;
   start-to-end puts the second path first. Default: the rule above (`a`'s
   path survives with its direction; `b`'s path is reversed when needed). The
   PO should reword AC 9 and say that "first" means the first-selected node.
3. **AC 14 contradicts itself.** "One ending the new open path and one
   starting it" and "one more node than before" mean the copies are the
   endpoints; "starting at the node immediately after the split point" reads
   as excluding them. Default: the copies are the endpoints. PO rewording.
4. **Gap: Join on the two ends of a 2-anchor open path.** The result is a
   1-anchor closed path that draws nothing. Default: Join is disabled for it.
   The PO should add this to AC 8.
5. **Gap: converting a node to the kind it already has.** Default: no write
   (above). This changes what `Make symmetric` does on an already-symmetric
   node (today it resets the handles), which slice 2's AC 11 never specified.
6. **For the tester:** all four endpoint combinations of AC 9; an Asymmetric
   drag to the node itself and with a zero-length opposite handle; reopen a
   `paths_v2.vmf` with `"smooth"` and check it reads as Symmetric; a
   multi-path node drag is one commit.
7. **No new crate, no new dependency, no ADR amendment.**
