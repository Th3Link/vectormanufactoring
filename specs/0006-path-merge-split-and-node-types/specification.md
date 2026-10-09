# Path merge/split and a third node type

Status: Done
Done, with: criterion 15 (both nodes selected after Split) is superseded by `edit-interaction-polish` criteria 50 to 52 (one node selected).
Priority: Must
Origin: Customer

## User value

As a maker I want to merge two path endpoints into one continuous path, split
a path apart at a chosen node, and switch a node between symmetric-round,
asymmetric and pointed (corner) behaviour, so that I can reassemble geometry
drawn in separate pieces or clean up after a mistake, without redrawing, and
without needing exactly-equal handle lengths on a curve that should only be
tangent-continuous.

**A naming note the architect and implementer must both read before touching
`AnchorKind`.** `path-node-editing` (slice 2) shipped exactly two node kinds,
`Corner` and `Smooth`, and its own PO write-up says plainly that it "merged
Inkscape's three node types (cusp/smooth/symmetric) into two (corner/smooth)
— dropped the smooth-vs-symmetric distinction (independently-adjustable
handle length vs. always-equal)." This slice un-defers exactly that
distinction. Inkscape itself names the two tangent-continuous kinds
confusingly from our point of view: Inkscape's "symmetric" node is the
equal-length one — which is what our existing `Smooth` already does — and
Inkscape's own "smooth" node is the independent-length one we are adding now.
Reusing the word "Smooth" for the new kind would collide both with our own
shipped kind's name and with Inkscape's overloaded terminology. This
specification uses three names, and no code, menu label or later spec may
reintroduce a fourth or reuse "Smooth":

- **Corner** — existing, unchanged. Independent angle and independent length
  on its two handles (cusp).
- **Symmetric** — existing, renamed from `Smooth`. Tangent-continuous
  (handles collinear through the node), equal length on both sides.
- **Asymmetric** — new. Tangent-continuous, like Symmetric, but each handle's
  length is independently adjustable.

**What "merge" and "split" are, and are not.** The customer's "merge" is
Inkscape's node-tool "Join selected endnodes" (`Shift+J`): select two
endpoint nodes and the tool joins them into one continuous path by moving
them together into a single node. It is not `boolean-operations`
(`specs/README.md`) — no union/difference/intersection, no winding rule, no
requirement that either path be closed — and it is not Inkscape's "Combine"
(`Ctrl+K`), which bundles multiple objects into one multi-subpath object
without moving anything (compound paths are explicitly out of scope for this
product per `path-node-editing`'s own "Out of scope"). "Split" is Inkscape's
"Break path at node" (`Shift+B`): select one node and the tool turns it into
two coincident end nodes, breaking one path into two pieces. The two
operations are exact inverses of each other, and this specification builds
them that way: Split's handle/kind rule is Join's rule run backwards.

## Acceptance criteria

### Node types: Corner, Symmetric, Asymmetric

1. Given a node of the kind `path-node-editing` built and called "smooth" in
   its own UI (tangent-continuous, equal-length handles, AC9/AC11 of that
   slice), then every maker-visible label for it — the node-tool toolbar
   button, its context-menu entry, and any tooltip — reads "Symmetric", not
   "Smooth"; its behaviour (equal-length mirrored handle drag, conversion
   to/from Corner) is unchanged. This is a label-only change: no new
   mechanics are tested by this criterion.
2. Given a selected Corner node, when the maker chooses "Make asymmetric"
   (a third option alongside the existing "Make corner" and "Make symmetric"
   from criterion 1), then two handles are set collinear through the node
   along the path's local tangent — the same tangent rule "Make symmetric"
   already uses — and each handle's length is: its own current length, if
   that side already had a non-zero handle; otherwise the slice's existing
   default handle length. The two resulting lengths may differ from each
   other; that is the one new thing this conversion can produce that "Make
   symmetric" cannot.
3. Given a selected Asymmetric node, when the maker drags one of its two
   handles, then the opposite handle rotates to stay collinear through the
   node at the dragged handle's new angle, but its own distance from the
   node does not change — only the dragged handle's own length changes.
   (Contrast: on a Symmetric node the opposite handle's length would also
   change to match, per that kind's existing drag rule; on a Corner node the
   opposite handle would not move at all.)
4. Given a selected Symmetric node, when the maker chooses "Make asymmetric",
   then both handles stay exactly where they are (same angle, same lengths)
   and only stop being forced equal going forward — shape-preserving, the
   same pattern as the existing Symmetric-to-Corner conversion, one notch
   less strict. Given a selected Asymmetric node, when the maker chooses
   "Make corner", then both handles likewise stay exactly where they are and
   stop being forced collinear — identical in effect to today's
   Symmetric-to-Corner conversion, now also reachable from Asymmetric.
5. Given a selected Asymmetric node, when the maker chooses "Make symmetric",
   then both handles are reset to the slice's existing default handle
   length, collinear through the node along the local tangent — the same
   result as today's Corner-to-Symmetric conversion; any existing
   independent lengths are discarded, not averaged.

### Getting two path objects' nodes into one Node-tool session (needed for Join)

6. Given the Select tool active with two or more path objects selected
   (shift-click, `canvas-navigation-and-selection`'s existing multi-select),
   when the maker switches to the Node tool (rail click or the `N`
   shortcut), then every one of those selected path objects shows its nodes
   and is editable in the same Node-tool session — this extends
   `path-node-editing`'s single-path node display to the multi-object
   selection `canvas-navigation-and-selection` already added, inventing no
   new selection mechanism. (Double-clicking one path with the Select tool,
   `canvas-navigation-and-selection` AC22, remains the one-object shortcut
   into the same state and is unaffected.)
7. Given two or more path objects' nodes visible per criterion 6, when the
   maker clicks one node and then shift-clicks a second node on a
   *different* visible path, then both nodes are selected together, exactly
   as `path-node-editing` AC10 already defines for two nodes — that
   criterion's wording never restricted shift-click-to-add to one path; this
   criterion confirms the same rule also holds across path objects.

### Join (merge two endpoints)

8. Given the node tool active, when the current node selection is exactly
   two nodes and both are endpoint nodes of open paths (the first or last
   anchor of a path whose closed flag is false) — whether both ends belong
   to the same open path or to two different open path objects — then the
   "Join" action (toolbar button and context-menu entry) becomes available.
   Given any other selection — not exactly two nodes, an interior node, any
   node belonging to a closed path, or the first and last anchor of the same
   open path when that path has exactly two nodes in total (joining them
   would collapse the whole path down to one node with nothing left to
   render) — "Join" is disabled, following `path-node-editing`'s existing
   disable-don't-hide convention for node-tool actions.
9. Given exactly two endpoint nodes selected, each the endpoint of a
   different open path object, when the maker triggers "Join", then the two
   path objects become one path object and the second is removed from the
   document; the two endpoints merge into one node positioned at the
   midpoint of their two prior positions, regardless of how far apart they
   were — there is no distance limit and no snapping; that node's two
   handles are, on each side, exactly the handle (possibly the zero vector)
   that side's original endpoint already had, unchanged in direction and
   length apart from moving with the node; and the merged node's kind is
   Corner, regardless of either original endpoint's kind. "First" and
   "second" below mean **selection order** (which node the maker clicked
   first vs. shift-clicked second), not path order or any other notion of
   first:
   - the first-selected node's path always keeps its own original node
     order — it is never reversed;
   - if the first-selected node is its path's *last* anchor, the result is
     that path's nodes (unchanged order) followed by the second-selected
     node's path's nodes — reversed if the second-selected node is *that*
     path's last anchor, left unchanged if it is that path's first anchor —
     so the two selected nodes become adjacent;
   - if the first-selected node is its path's *first* anchor, the result is
     the second-selected node's path's nodes — reversed if the
     second-selected node is that path's first anchor, left unchanged if it
     is that path's last anchor — followed by the first-selected node's
     path's nodes (unchanged order);
   - reversing a path's node order also swaps each of its nodes' incoming
     and outgoing handle (so every segment's curvature is preserved in the
     new traversal direction); positions and handle lengths are untouched
     by a reversal.
10. Given exactly two endpoint nodes selected, being the first and last
    anchor of the same one open path, when the maker triggers "Join", then
    that path becomes one closed path object with one fewer node than
    before — the two endpoints merge into one node by the same
    midpoint-position, handle-preserving, Corner-kind rule as criterion 9.
    This is the same end state as `path-node-editing` AC5's close-while-
    drawing, reached here after the fact through the node tool.
11. Given a successful Join (criterion 9 or 10), then only the newly merged
    node is selected afterward — the two prior selections collapse into
    one — so the maker can immediately continue working at the junction
    (e.g. convert it to Symmetric or Asymmetric) without re-selecting.

### Split (break a path at a node)

12. Given the node tool active, when the current node selection is exactly
    one node, and that node is either an interior node of an open path (not
    its first or last anchor) or any node of a closed path, then the "Split"
    action becomes available. Given any other selection — zero or more than
    one node selected, or the first/last anchor of an open path, which has
    nothing on one side to split off — "Split" is disabled.
13. Given one interior node of an open path selected, when the maker
    triggers "Split", then the one path object is replaced by two separate
    open path objects: the first holds every node from the original path's
    start up to and including a copy of the selected node; the second holds
    a second copy of the selected node followed by every node after it to
    the original path's end. Both copies sit at the original node's
    position. The first copy keeps the original node's incoming handle and
    has its outgoing handle set to the zero vector (retracted); the second
    copy keeps the original outgoing handle and has its incoming handle set
    to the zero vector. Both copies' kind becomes Corner, regardless of the
    original node's kind — the exact reverse of criterion 9's merge rule.
14. Given one node of a closed path selected, when the maker triggers
    "Split", then that one path object becomes a single open path object
    with one more node than before: the selected node is replaced by two
    copies at the same position, using the same handle/kind rule as
    criterion 13 — the copy that keeps the original outgoing handle (zeroed
    incoming handle) and the copy that keeps the original incoming handle
    (zeroed outgoing handle). Those two copies are themselves the resulting
    path's two endpoints, not separate nodes next to them: the
    outgoing-handle copy is the new path's first node, the incoming-handle
    copy is its last node, and the path is traversed starting at the
    outgoing-handle copy, through every other original node in their
    original cyclic order, ending at the incoming-handle copy.
15. Given a successful Split (criterion 13 or 14), then the two resulting
    coincident nodes are both selected afterward — on two separate path
    objects for criterion 13, on the two ends of one open path for criterion
    14 — so the maker can immediately re-Join them without re-selecting; to
    instead separate the pair, the maker clicks empty canvas first (clearing
    the two-node selection), then clicks and drags just one of the two
    coincident nodes, which moves only that node away from its still-
    stationary twin.
16. Given a selected node, when the maker chooses the conversion action for
    the kind that node already has (e.g. "Make asymmetric" on a node already
    Asymmetric, "Make symmetric" on a node already Symmetric, "Make corner"
    on a node already Corner), then nothing happens: no handle, kind or
    position changes, and no document commit is made. **This amends
    `path-node-editing`'s AC11**, under which re-applying "Make smooth" to
    an already-smooth node reset its handles to the tangent/mirrored default
    every time it was clicked; from this slice onward, re-applying any of
    the three conversion actions to a node already of that kind is a no-op
    instead. Stated explicitly here because it is a deliberate, small
    behaviour change from what shipped in slice 2, not an oversight — the
    implementer must not carry the old reset-on-reapply behaviour forward.

## Out of scope

- Joining any node of a closed path. A closed path has no endpoints; AC8
  excludes it from Join's selection entirely. A maker who wants to open a
  closed path first uses Split (criterion 14), not Join.
- Joining more than two endpoints in one action. Inkscape pairs up selected
  endpoints by proximity when more than two are selected; this slice's Join
  (AC8) only ever considers exactly two.
- Inkscape's "Join selected endnodes with a new segment" — a second join
  variant that adds a straight connecting line and keeps both original
  nodes in place rather than merging them into one. This slice ships only
  the merge-to-one-node behaviour (criteria 9-10).
- Hovering a specific endpoint so the merged node lands exactly there instead
  of at the midpoint (an Inkscape nicety on top of `Shift+J`). This slice's
  Join always uses the midpoint.
- Split producing anything other than exactly two pieces from one selected
  node — no breaking at several selected nodes in one action, and no
  "Delete segment" (a different Inkscape action that removes a segment
  rather than duplicating a node).
- Any boolean-operation-like behaviour. Join and Split never evaluate
  overlap, winding or fill between two paths' shapes — that is
  `boolean-operations`'s job, on closed paths, and is entirely separate from
  these two open-endpoint operations.
- Clicking directly on a second path's outline while already in the Node
  tool, to add it ad hoc to the current editing set. This slice's only path
  into multi-path node editing is criterion 6: select the objects with the
  Select tool first, then switch to the Node tool.
- Undo/redo of Join, Split, or any node-kind conversion in this slice —
  `undo-redo`, the same deferral `path-node-editing` already stated for its
  own operations.
- Keyboard shortcuts for Join, Split or "Make asymmetric" — none of them has
  a single obvious letter the way `B`/`N`/`S` do. Toolbar and context-menu
  only, same reasoning `path-node-editing`'s node-tool-actions note already
  gives for its own three actions.
- Rubber-band/marquee selection of nodes or objects — already deferred by
  `path-node-editing` and `canvas-navigation-and-selection`; unchanged here.

## UX notes

**Sizing baseline for everything below:** PR #20 (`fix/canvas-interaction-
bugs`) has since merged to `main` and doubled the node-tool glyphs: node
glyph 7px→14px, node/handle hit-test radius 8px→16px, node hover ring
10px→18px. `docs/design-system.md` on `main` reflects these current
numbers. Everything below is sized against the 14px node glyph, 16px hit
radius, 18px hover ring.

### Third node glyph: Asymmetric = triangle

Corner is a 14×14px axis-aligned square; Symmetric is the same 14×14px
glyph rotated 45° to a diamond — both four-sided, differing only by
rotation. A third quadrilateral (e.g. a non-square rhombus, or the same
diamond at a different rotation) would be the wrong move at this larger
size: two 14px four-cornered shapes that differ only by angle are already
about as close as I'd want two glyphs to sit, and a third one in the same
family invites "is that rotated 40° or 50°?" squinting on a dense path.

**Asymmetric node: an equilateral triangle, point-up, inscribed in the same
14×14px screen-space box, same fill/stroke states as the other two** (white
fill + `--node-stroke` outline unselected, solid `--accent` filled
selected). Three corners vs. four is a genuine silhouette difference, not
a rotation of the same polygon, so it reads at a glance even in a cluster
of nodes and even before color communicates selection state. It also
doesn't collide with the one *curved* glyph already in the vocabulary —
the 12px handle-endpoint circle, which only appears on a selected node's
handles. Node-kind glyphs stay "polygon" (square, diamond, now triangle);
the handle stays the one circle. That boundary is incidental today but
worth keeping, since it's one more thing that keeps the two vocabularies
from blurring together the way `docs/design-system.md` already insists on
elsewhere (`primitive-shapes`' shape handle is deliberately square and
never circular/diamond for the same reason).

I did check whether Inkscape's own node-type icons give us a ready-made
third shape to adopt directly (the way we're adopting `Shift+J`/`Shift+B`
for Join/Split's naming, and did for `B`/`N` earlier). It doesn't: Inkscape
renders both its "smooth" and "symmetric" kinds as the same on-canvas
diamond and distinguishes them only in the node-tool's status bar text and
toolbar button state, never with a third on-canvas shape. There's no
existing convention to borrow here, so the triangle is this project's own
call, not a parity pick.

`docs/design-system.md`'s "Node glyph" row (Spacing and sizing table)
needs its description cell extended to list all three shapes; I've done
that below. The value cell is left at whatever `fix/canvas-interaction-bugs`
lands with — not re-litigating that PR's numbers here.

### Contextual toolbar/context-menu: segmented 3-way kind control, plus two new action buttons

**The existing "Make corner"/"Make smooth" pair (slice 2's `NodeToolbar`,
renamed "Make symmetric" per criterion 1) becomes a 3-segment `ToggleGroup`:
Make corner / Make symmetric / Make asymmetric**, using the node glyphs
themselves (square/diamond/triangle) as the segment icons rather than
words — this is exactly the "small set of mutually-exclusive icon choices,
≤4, icons read faster than words" case `docs/design-system.md`'s
Interaction Conventions section already names the `ToggleGroup` pattern for
(precedent: `primitive-shapes`' Polygon/Star toggle, generalized there for
join/cap/fill-mode). The segment matching the selected node's current kind
renders active (filled `--accent`, the same "active" look every other
`ToggleGroup`/active-tool-button uses) — clicking the already-active
segment is an inherent no-op of the control itself, so no separate
disabled state is needed there, unlike Join/Split below. This replaces two
flat buttons with one 3-segment group in the same mini-toolbar slot;
Insert node / Delete node / Make line / Make curve are unrelated choices
(not mutually exclusive alternatives of one property) and stay as
individual icon buttons, unchanged.

Right-click context menu: same three entries (Make corner / Make symmetric
/ Make asymmetric), unchanged in form from today's two-entry version — a
flat menu, not a `ToggleGroup` (that control doesn't exist in menus). The
entry matching the node's current kind is disabled (greyed, not hidden),
reusing the already-established disable-don't-hide convention for a
clicking-it-would-be-a-no-op case, same reasoning as Join/Split's
disabled state below.

**Join and Split are two new individual icon buttons**, added to the same
floating mini-toolbar and to the same context menu, alongside the existing
actions — not part of the kind `ToggleGroup`, since they're one-shot
operations, not a mutually-exclusive property. Both follow AC8's own
stated disable-don't-hide rule (see "Error/no-op feedback" below).

### Join trigger: toolbar button + context-menu entry, no keyboard shortcut

The specification's own "Out of scope" section already decides this —
"Keyboard shortcuts for Join, Split or 'Make asymmetric'... Toolbar and
context-menu only" — so there's no open question for me to re-decide here,
and I'm not going to quietly override a PO scope call from the UX-notes
section. For the record, since the brief raised it directly: I'd have
leaned toward adopting Inkscape's `Shift+J` anyway, for the same
muscle-memory-parity reason this project already adopted `B`/`N`/`S`/`R`/
`E`/`*` verbatim — but the spec's own reasoning (no single obvious letter,
and the precedent `path-node-editing` already set for Insert/Make-line/
Make-curve) is sound on its own terms, and `Shift+J` isn't "a single
obvious letter" the way the existing bindings are. If the customer wants
it reconsidered, that's a PO scope conversation, not a UX one. **Trigger:
mini-toolbar icon button + context-menu entry, both disabled unless the
selection qualifies (AC8). No keyboard shortcut in this slice.**

### Split trigger: toolbar button + context-menu entry, no keyboard shortcut

Same answer, same reasoning, mirrored from the same "Out of scope" line
(`Shift+B` not adopted here for the same reason `Shift+J` isn't). **Trigger:
mini-toolbar icon button + context-menu entry, both disabled unless the
selection qualifies (AC12). No keyboard shortcut in this slice.**

### Visual feedback for Join: instant, no animation

The merged node simply appears at the computed midpoint the instant Join
is triggered — no tween/ease-toward-each-other. Nothing else discrete in
this product's node-tool animates: Make corner/Make symmetric/Make
asymmetric conversions, Insert, and Delete (`path-node-editing`) all apply
instantly, and the only motion anywhere in this UI is either a live drag
(continuous, mouse-driven) or a hover/selection color change. Adding a
tween for exactly this one command would be a new animation vocabulary for
no stated reason — "keep it simple" wins here. The two prior node
selections collapse into the one new selection per AC11, rendered with the
ordinary selected-node glyph (filled `--accent`) — no extra "just merged"
flourish.

### Visual feedback for Split: yes — both new nodes render selected

Already stated by the spec itself (AC15: "the two resulting coincident
nodes are both selected afterward"), so this one isn't open either; noting
the rendering consequence since the brief asked for it explicitly. Both
copies sit at the exact same position (AC13/14), so immediately after
Split they render as two fully-overlapping selected-node glyphs — visually
indistinguishable until the maker drags one away, which is expected and
fine: the existing "every selected node shows the identical selected
glyph, no primary/secondary distinction" rule
(`specs/0002-path-node-editing/specification.md`'s UX notes) already
covers this without needing a new rule for the coincident case. The
"something just split" signal is the mini-toolbar/context-menu now
reading as a two-node (or cross-object) selection rather than one, plus —
once the maker's first drag separates them — two visibly distinct paths/
nodes where there was one. No separate flash/highlight on top of the
existing selected-glyph treatment.

### Error/no-op feedback: disable-don't-hide, no message

Same convention as every other node-tool action, and AC8 already states
it explicitly for Join ("following `path-node-editing`'s existing
disable-don't-hide convention for node-tool actions"); I'm extending the
identical rule to Split (AC12 doesn't restate it, but there's no reason to
invent a second convention for the operation that's explicitly defined as
Join's exact inverse). Concretely: the Join and Split buttons (mini-toolbar
and context menu) are always visible but greyed out/non-interactive
whenever the current selection doesn't qualify — wrong node count, an
interior/closed-path node for Join, or a closed-path non-endpoint... for
Split's complement — exactly mirroring how Insert/Delete/Make-line/
Make-curve already disable rather than hide. No toast, no inline error
message, no modal. This also matches `primitive-shapes`' contextual-action
enablement pattern the brief pointed at — same instinct, applied to the
same kind of "selection doesn't support this action right now" state
everywhere in this product rather than inventing a messaging pattern for
just these two.

### `docs/design-system.md` additions

- **Node glyph** row (Spacing and sizing table): description cell becomes
  "Corner (square), Symmetric (diamond) and Asymmetric (triangle) node
  markers" — value cell untouched here (owned by
  `fix/canvas-interaction-bugs`).
- **Segmented icon control (`ToggleGroup`)** bullet (Interaction
  conventions): add the node-kind 3-way (Corner/Symmetric/Asymmetric) to
  the list of qualifying examples alongside Polygon/Star and join/cap/
  fill-mode.
- No new color or size tokens needed for Join/Split — both reuse the
  existing mini-toolbar icon-button styling and the existing disabled-state
  treatment.

## Links
Requirements: R-EDIT-014, R-EDIT-001 (`docs/requirements.md`)

**Flag for the architect — this is a real `AnchorKind` enum change, not an
additive one.** Today's schema (`specs/0002-path-node-editing/adrs.md`,
"the anchor schema") stores `kind: Corner | Smooth` as an LWW register.
Criterion 1 renames the existing `Smooth` value and criterion 2-5 add a
third value (`Asymmetric`) alongside it — whatever the wire/enum
representation, every already-written document's `Smooth` value must still
read as the equal-length kind after this change, under whatever name the
implementer picks internally. ADR 0004 §9's versioning rule means this needs
a `format_version` bump (the document model widens); the exact migration
mechanism (rename-in-place vs. new variant plus a reader fixup) is the
architect's and implementer's call, not decided here. Flagging only that the
weight exists, matching how `path-node-editing`'s own `adrs.md` flagged its
anchor-schema decisions to the lead.
PR: https://github.com/curvyo/curvyo/pull/26
