# ADRs for "Multi-object transform: one group box with the same handles as a single object"

This feature adds no stored field. Every write goes to a register the
single-object gestures already write: the primitive frame, `corner_radii`,
`rotation`, `stroke_width`, path anchors and handle vectors. The group box,
the group drag and the typed entries are ephemeral UI state. **No new crate,
no new external dependency, no new `curvyo-geometry-core` function, no ADR
amendment, no `format_version` change.** `curvyo-document-core` gains one
command (`Document::transform_objects`) and two snapshot methods
(`PathSnapshot::scaled_along`, `sheared_along`).

Reference state: `main` at `177cd16` plus `story/advanced-selection` (PR #61,
not merged yet). Every PR of this feature starts after #61 merges.

Six criteria contradict accepted behaviour or cannot be met as written. They
are under "Flagged to the lead", each with the default this file builds
against.

## Depends on

- [ADR 0001 §1, §3, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md)
  and [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md): group
  box, handle layout, gesture arithmetic and press order in `ui-core`; drawing
  from `DecorationInput` in `render-core`; binding in `editor-wasm` and
  `frontend/`. Every piece fits an existing crate over an existing edge.
- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  the per-node affine is still not built. A group map is applied to each
  object and baked into its own registers, as rotation and skew already are.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one gesture is one commit, for all selected objects together (criterion 31).
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md): the
  group box and the drag are ephemeral; every written field keeps its
  per-field LWW register, so the merge rules of the single-object gestures
  hold object by object.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md): no
  new key and no new meaning of an existing key, so no bump (decision 5).
- [`specs/0005-object-transform/adrs.md`](../0005-object-transform/adrs.md):
  `rotation` register, merge-granularity table, `ObjectSnapshot::rotated`,
  "the oriented box lives in `ui-core`", the √(sx·sy) stroke and radius
  factor, the 0.01 mm stroke floor.
- [`specs/0008-object-transform-refinements/adrs.md`](../0008-object-transform-refinements/adrs.md):
  one resolving function per gesture shared by preview, release and entry;
  skew writes anchors only; the snap table.
- [`specs/0009-unified-object-editing/adrs.md`](../0009-unified-object-editing/adrs.md):
  the blue overlay built from `select_view::live_objects`, the 8 ms benchmark
  and its reference desktop.
- [`specs/0010-edit-interaction-polish/adrs.md`](../0010-edit-interaction-polish/adrs.md):
  `commit_move` and `Document::duplicate_objects` (copy above its own
  original, ids returned in source order), the typed move, `classify_press`.
- [`specs/0014-advanced-selection/adrs.md`](../0014-advanced-selection/adrs.md): Alt at
  the press wins over every target; Shift and Ctrl inside the box arm the
  marquee; modifiers reach the session through `Session::held`.
- [`specs/0007-stroke-and-fill-styling/`](../0007-stroke-and-fill-styling/)
  criterion 29 (option B): applies unchanged; it is not extended to a
  selection (question 3 decided (b), criterion 44 removed).

## Feature-local decisions

- **2026-10-08: (1) where the group transform lives, and one commit for N
  objects.**
  - **The group box is an `OrientedBox` with angle 0.** New module
    `ui-core/src/group_box.rs`: `group_bounds(objects, selection) ->
    Option<OrientedBox>`, the union of `object_outline_bounds` (criterion 1;
    the same bounds as the typed absolute move), plus the two selection tests
    `is_aligned_primitive` and `is_uniform_only` (criteria 20, 21, D3). Because
    the box is an `OrientedBox`, the existing handle layout and hit test
    (`transform_handle_layout`), `resize_local_box`, `resize_anchor_local_position`,
    `polygon_star_resize_factor`, `rotate_pivot`, `rotate_delta_angle`,
    `skew_frame` and `skew_angle` serve the group unchanged. The group's
    `HandleSpec` is computed from the selection: corner resize only when
    uniform-only, skew only when every object is a path, never parameters,
    and the degenerate-axis rules of criteria 12 and 13.
  - **The gesture is computed once, applied N times.** New module
    `ui-core/src/group_transform.rs` (responsibility: apply one document-axes
    map to one object, kind by kind). A crate-private
    `enum GroupMap { Translate(Vec2), Scale { pivot, sx, sy }, Rotate { pivot,
    delta }, Shear { fixed, ku, kv } }` and `fn apply(object, map, modes) ->
    ObjectSnapshot`. Not a public affine type (0005's rule). New module
    `ui-core/src/select_tool/group_drag.rs`: `GroupDrag { origin, starts:
    Vec<ObjectSnapshot>, start_box, handle, modes }` with one
    `resolve(current, shift, ctrl) -> Vec<ObjectSnapshot>` that derives the
    `GroupMap` from the start box and maps every start snapshot. Preview,
    release and typed entry call it, so they cannot disagree (criterion 32).
    `SelectDrag` gains one variant, `GroupTransforming(GroupDrag)`.
  - **Sanity is all-or-nothing.** If any mapped object fails `is_sane`, the
    whole resolution is the start state (no change). Per-object `sane_or`
    would leave some objects behind, which criterion 31 forbids.
  - **Commit: one new command, not a transaction mode.** Options:
    - (A) `Document::transform_objects(results: &[ObjectSnapshot],
      write_stroke_width: bool) -> Result<(), ObjectEditError>`. Chosen. It
      resolves every id, kind and anchor before the first write, then writes
      per object only what differs from the stored value: frame,
      `corner_radii`, `rotation`, the anchors' point and handle vectors, and
      the stroke width only with `write_stroke_width`. One
      `commit_with_label("transform_objects")`. A stale id, a changed kind or
      a missing anchor refuses the whole call, as `translate_objects` does. A
      stroke width that is not writable refuses before any write; `ui-core`
      retries once with `false`, the existing `write_with_width_fallback`
      rule. The per-kind field writers of `resize_rect`, `resize_ellipse`,
      `resize_star_frame`, `resize_path` and `rotate_object` move into
      private non-committing helpers that both the old commands and the new
      one call, so every field is written by one function. The single-object
      commands keep their behaviour (criterion 52).
    - (B) A batch guard (`document.batch(|d| ...)`) that suppresses the
      commits of the existing commands. Rejected: hidden mode state on
      `Document`, and a refusal of object k leaves writes 1..k-1 pending for
      the next commit, so it is not atomic without the pre-resolution (A)
      does anyway. Grouping of commits is `undo-redo`'s question.
    - (C) N commits. Rejected: criterion 31; a peer could import a prefix,
      a later undo would need N steps, and a commit has fixed overhead.
  - **Move and copy keep their commands.** A group move is
    `translate_objects`, a copy is `duplicate_objects` through `commit_move`;
    both are already one commit for any number of objects.
  - **Anchor resolution in `transform_objects` is linear.** `anchor_index` is
    a linear scan, so the per-anchor lookup of `resize_path` is quadratic per
    path. The new command builds one id-to-index map per path.

- **2026-10-08: (2) primitives under a group map.**
  - **Path, scale and shear in document axes.** `PathSnapshot::scaled` and
    `sheared` work in the path's own frame (`rotation`). New
    `scaled_along(pivot, sx, sy, axes: Angle)` and `sheared_along(pivot, ku,
    kv, axes)`; the old methods become `*_along(.., self.rotation)`. The group
    passes angle 0. `rotation` is unchanged (criterion 20). Pure arithmetic,
    no command.
  - **Rotate** is `rotate_by` (`ObjectSnapshot::rotated`) for every kind, as
    today (criterion 26).
  - **Primitive scale** is a new function in `group_transform.rs`: frame
    centre mapped about the pivot; a uniform factor multiplies every size; an
    aligned rectangle or ellipse with sx ≠ sy takes sx on the dimension along
    document x and sy on the one along y (swapped at ±90°); polygon and star
    take the one factor on the outer radius. A circle (rx = ry within the
    tolerance) at a rotation that is not a multiple of 90° becomes an ellipse
    with `rotation` 0 when sx ≠ sy. That is the one new write (flag 4).
  - **Kinds are preserved; no conversion.** Non-uniform scale of a
    uniform-only selection never reaches the per-object function: the edge
    handles are absent and a corner drag uses `polygon_star_resize_factor`.
    Skew never reaches a primitive: the skew handles are absent.
  - **Stroke and radius** use `scale_stroke` and the existing radius
    multiplier with `stroke_or_radius_factor(sx, sy)`, from the modes copied
    at the press. Each of the four `corner_radii` is multiplied by the factor
    (`rectangle-corner-radii` has shipped).

- **2026-10-08: (3) group bounds, cost, caching, budgets.**
  - **Bounds.** `object_outline_bounds` serves the group box at 10,000
    objects: a path is one `segment_bounds` per segment, a primitive one
    small outline (8 anchors at most). Estimated a few milliseconds natively
    for 10,000. **No cache and no incremental update.** The box is computed
    once per call from the objects slice the caller already holds (draw,
    hover, press). `GroupDrag` freezes the start box at the press. During
    scale and skew the preview box is recomputed from the preview objects
    (criterion 29); during a rotate it is the start box turned (criterion 28).
    A cache would only pay off once `Session::objects()` stops reading every
    object on every call (below).
  - **Where the time goes at 10,000.** `docs/technical-debt.md` measures
    14 to 18 ms per frame at rest for 200 objects, almost all of it
    `Session::objects()` reading the document and the uncached renderer
    rebuilding the draw list. Linear extrapolation gives roughly 0.7 to 0.9 s
    per frame at rest for 10,000 objects, before this feature adds anything.
    Criterion 49's end-to-end numbers cannot be met by this feature. They
    need the draw-list and snapshot cache by document version ("Canvas
    performance", resolution), which belongs in its own `chore/`.
  - **Budgets, replacing criteria 48 and 49 (D7).** Reference desktop: the
    customer's Linux machine (Tauri, WebKitGTK, WebGL2) for one frame-rate
    check on the real window. Gates: `#[ignore]` benchmarks in a native
    release build on the build host, as in `unified-object-editing`.
    - 200 objects (criterion 48): a group scale, rotate or skew preview frame
      of `Session::draw_list()` takes at most 1.1 times the plain move frame of
      the same selection in the same run. The group box of the 200 objects
      takes under 1 ms (not 5 ms; expected about 0.1 ms). The absolute 8 ms
      stays the target of the cache item and is not a gate here: the plain
      move misses it today (13.8 to 15.6 ms).
    - 10,000 objects (criterion 49): the group box takes under 20 ms; one
      `GroupDrag::resolve` plus the preview box takes under 50 ms; a
      `transform_objects` commit (scale, rotate, skew) and a
      `translate_objects` commit each finish within 5 s. Measured and
      reported in the PR, not gated: the copy commit, the WASM commit times,
      the end-to-end pointer-to-frame time and the at-rest frame, and the
      bytes a 10,000-object scale adds to `document.loro`.
    - **No simplified preview (confirmed 2026-10-08, criteria 28, 49).** The
      blue overlay stays at every count; it is the only feedback on the
      shapes. Above 500 selected objects the member boxes are not drawn
      (criterion 5, one constant `PER_OBJECT_BOX_LIMIT`); that is a display
      rule, not a preview simplification. A threshold is introduced only if a
      measurement asks for one, with the UX notes' section 12 rule.

- **2026-10-08: (4) press order, modifiers, copy, typed entries, snapping.**
  - **Press order (criteria 14, 16, 43, 45, 46; question 3 decided (b),
    2026-10-08).** The group box has no hit area of its own. `classify_press`
    for a multi-selection adds exactly one step to today's order: after Alt
    (`PressTarget::Lasso`, unchanged), the drawn group handles and the centre
    handle (its hover region) map to `PressTarget::Handle` and
    `PressTarget::CentreHandle` from the group box. Everything after that is
    today's multi-selection path, unchanged: `hit_test_object` (8 px outline
    or filled interior, `0007` option B as it is), where a selected object
    begins a move of the selection and an unselected one replaces it
    (`begin_object_press`); Shift toggles or axis-locks, Ctrl copies on an
    object; no hit arms the marquee (Shift adds, Ctrl removes) and a click
    clears at release. `InsideSelectedBox`, `is_inside_selected_box` and
    `filled_interior_above` stay sole-selection rules and are not extended to
    a group. A handle that is not drawn has no hit area, so a press there
    falls through to the ordinary order (criterion 14); the one exception is
    the edge resize handles under 24 px, which `transform_handle_layout`
    already hit-tests. Hover (criterion 46) is today's object hover plus the
    group handle cursors. The copy badge reads the same `classify_press`, so
    it cannot disagree.
  - **`wait` cursor (criteria 28, 46, 49).** The commit is synchronous on the
    main thread. The frontend sets `wait` and keeps the last preview frame on
    release, calls the commit, and renders the committed state after it
    returns; no frame of old geometry is drawn in between.
  - **Ctrl copy** is `commit_move(.., copy: true)` with the selection's ids:
    every copy sits directly above its own original, the ids come back in
    selection order and become the selection (criterion 17). Unchanged code.
  - **Keys M, R, S, K** keep the gate of `edit-interaction-polish`
    criterion 55; `open_entry_for_key` stops refusing a multi-selection. The
    group entries hold a `GroupDrag`-shaped start (snapshots, box, handle,
    pivot fixed at open) and resolve through the same `GroupMap` (criteria 33
    to 36). The typed rotate is relative: `Rotate { delta: typed }`. The typed
    move is `commit_move`; Absolute uses the group box's top-left.
  - **Snapping.** Ctrl on a group rotate uses `rotate_delta_angle` (relative,
    from 0), never `rotate_delta_for`, even when the selection holds
    polygons (criterion 27). Skew uses `skew_angle` with its ±75° cap.

- **2026-10-08: (5) file format and concurrency.**
  - **Format: verified, no change.** Every write is a key the single-object
    gestures already write, with the same meaning (anchors in document space,
    primitive frame in its local frame, `rotation` in radians normalized).
    `document.json` is unchanged; nothing about a group is stored
    (criterion 50). `format_version` stays.
  - **Atomicity.** One `commit_with_label` is one Loro change; a peer imports
    a change whole, and a save snapshots between commands, so no reader sees
    half a gesture.
  - **Merge.** Per object, the writes are those of the single-object gesture
    of the same kind, written only when changed, so 0005's merge table holds
    object by object. A peer's concurrent edit of one object can win that
    object's fields by LWW and leave the arrangement out of line; ADR 0009 §3
    accepts this, and presence is the mitigation.
  - **Stale objects.** A Select drag holds the snapshot it started with and
    no merge runs during a drag today (`drag_objects`, technical debt).
    Once sync reaches the session, one peer delete during a 10,000-object
    drag refuses the whole commit. Revisit with sync: skip missing objects
    instead of refusing.
  - **History size.** A 10,000-object scale writes about 300,000 register
    operations (three per anchor). It is one undo step later, and it feeds
    "Document files grow with edit history"; the PR reports the bytes.

- **2026-10-08: (6) module size, PR split, parallel work.**
  - **Size limits.** `editor-wasm/src/session/select_view.rs` is at 501
    non-test lines: task 1 of PR 1 is a pure-move split (decorations; cursor
    and hint; readout). `session/mod.rs` (499) gets no new field; the group
    box is derived, the drag lives in `SelectTool`. `ui-core`'s
    `transform_entry.rs` (548), `transform_drag.rs` (466), `transform_math.rs`
    (486) and `transform_handle_layout.rs` (481) do not grow: the new code
    goes into `group_box.rs`, `group_transform.rs`,
    `select_tool/group_drag.rs` and `select_tool/group_entry.rs`.
    `select_tool.rs` (408) gains one variant and its dispatch. New tests go to
    `curvyo-ui-core/tests/multi_object_transform.rs` and an `editor-wasm`
    test file, not into the 2,000-line test module of `select_tool.rs`. The
    `render-core` doc comment of `select_box.rs` ("never one merged box")
    changes with the group box.
  - **PR split, in order, each a `story/` PR after #61:**
    1. **Group box and move.** `ui-core` (`group_box.rs`, the group-handle
       step of `classify_press`, centre handle), `render-core` (group box,
       member boxes, the 500 cutoff), `editor-wasm`, frontend. Criteria 1 to
       8, 14 (centre handle), 16, 17, 35, 37 (M), 43, 45 to 47, 50 to 52. No
       handle other than the centre handle is drawn; the selection moves by
       the centre handle and by its selected objects (criterion 16).
    2. **Scale.** `document-core` (`transform_objects`, `scaled_along`),
       `group_transform.rs`, `group_drag.rs`, corner and edge handles.
       Criteria 9 to 15, 18 to 24, 29 to 32, 34, 37 (S), 38, 39, 48, 49 with
       the benchmarks.
    3. **Rotate.** `ui-core`, `editor-wasm`, frontend only. Criteria 25 to 28,
       33, 37 (R).
    4. **Skew (Part E).** `sheared_along` in `document-core`, skew handles.
       Criteria 36, 37 (K), 40 to 42 (question 4 decided (a), 2026-10-08).
  - **Parallel work (`CLAUDE.md` §4: not on the same crates).**
    `style-panel-rework` touches `ui-core` and `editor-wasm` in every PR, so
    no PR of this feature runs next to it. `stroke-markers` PR 1
    (`document-core`, `render-core`) may run next to this feature's PR 3
    only; its PR 2 (`ui-core`, `editor-wasm`) may not run next to any of
    them. Both style stories also edit
    `frontend/src/hooks/useEditorSession.ts`; expect a rebase. Recommended
    order: `style-panel-rework` (Must, Ready) first, then this feature.

- **2026-10-08: forward note for groups (Out of scope, first entry).**
  `group_transform::apply(object, map, modes)` is a function of one object
  and one document-axes map, so a later group object can call it with its
  own frame. Nothing about the ad hoc group is stored, so
  `layers-and-grouping` chooses its own storage (ADR 0002 §5) freely.

- **2026-10-10: (7) notes from the build (implementer).** None of them changes a
  decision above; they record what the build chose where the text left room.
  - **Public API of `curvyo-document-core`.** Besides `Document::transform_objects`,
    `PathSnapshot::scaled_along` and `sheared_along` (decision 1), the build added
    two variants to `ObjectEditError`, `InvalidStrokeWidth` and `InvalidRadius`,
    because the command refuses a bad width or radius before it writes
    (the `resize_*` commands refuse the same two with `ShapeEditError` and
    `PathEditError`). `ui-core` retries once without widths, as decided.
  - **Circles (criterion 20, D3).** A circle that is stretched (sx differs from sy)
    becomes an ellipse with `rotation` 0 whatever its rotation, as the table says;
    criterion 31's "not a multiple of 90 degrees" only names the case that goes
    beyond a single object's write. A circle scaled by one factor keeps its
    `rotation`.
  - **Uniform-only corner drag.** `resize_local_box` with Ctrl forced on: both
    factors come from the dominant axis of the drag, the same rule Ctrl gives a
    free corner, so the dragged corner follows the pointer on that axis (criterion
    29). A single polygon's diagonal rule is not used: a group has no one radius.
  - **Flat boxes.** `transform_handle_layout::hit_transform_handle` gained a sibling
    `hit_transform_handle_for_side` that takes the "shorter side" `s` as a
    parameter (the old function calls it with the box's shorter side), because a
    flat group box uses its other extent (criterion 12). The centre handle of a
    group is decided in `group_box.rs` with the group's own `s`.
  - **Press order.** `classify_press` adds the group handles as one early return
    for a selection of two or more; the objects, the marquee and the Alt cycle
    after it are the code of before.
  - **Keys.** `KeyHint::SelectOne` and `KeyEntryRefusal::SeveralSelected` are gone
    (criterion 37). K or Shift+K on a selection that holds a non-path is
    `SkewNeedsPath`, as for one object.
  - **Draw order.** Member boxes are drawn with the group box, above the blue
    preview outlines (one `GroupDecorationInput`), not below them as UX notes
    section 12 lists; they are a hairline at 60% and the difference is not visible.
  - **`wait` cursor (decision 4).** `Session::release_is_slow` is true for a move or
    group transform of 100 or more objects; only then does the host show `wait`,
    yield one frame and release. Below that the release is immediate.
  - **Selection announcement (UX U4).** Built: `Session::selection_announcement` and
    a visually hidden polite live region, settled for 500 ms before it speaks.

## Flagged to the lead

*2026-10-08: flags 1 to 6 are applied to the criteria by the PO; flag 7 is
open for the lead.*

1. **Criterion 48 cannot pass as written.** "Within 8 ms" fails before this
   feature: the plain move preview of the same 200 objects takes 13.8 to
   15.6 ms (`docs/technical-debt.md`). Replaced by decision 3: 1.1 times the
   plain move, group box under 1 ms. The PO rewords.
2. **Criterion 49's end-to-end numbers depend on the draw-list cache.** About
   0.7 to 0.9 s per frame at rest at 10,000 objects is extrapolated from the
   measured cost, without this feature. Replaced by decision 3 (the feature's
   own costs gated; end-to-end reported). If the customer needs 10 fps at
   10,000, the cache `chore/` has to be scheduled first; that is the lead's
   call, not a customer question.
3. **Criteria 20 and 29, "exact image".** A rounded rectangle is not scaled
   exactly whenever its radius does not follow the outline: with "Scale corner
   radius" off (aligned or rotated), and with sx ≠ sy (a circular radius
   cannot follow two factors). The exception names only the rotated case. The
   group box still follows the pointer for an aligned rectangle, because its
   corners stay inside the frame. The PO widens the exception.
4. **Criterion 31, "nothing else is written".** A circle at a rotation that
   is not a multiple of 90°, stretched with sx ≠ sy, gets `rotation` 0 in
   the scale commit (D3 needs it). Built as written in decision 2; the PO
   adds the case to criterion 31.
5. **Criterion 22, "the factor stops at the value that reaches the limit."**
   The single-object rule is that an insane result resolves to no change
   (`sane_or`). Built as the single-object rule, all or nothing (decision 1);
   the PO rewords. Criterion 24's "(or radii once `rectangle-corner-radii`
   ships)" is stale: it has shipped (#53).
6. **Criterion 5 against `docs/design-system.md`** ("never a single merged box
   ... in its place"): above 500 objects only the group box is drawn. For the
   `ux-engineer` to amend the design system.
7. **For `docs/technical-debt.md` (lead):** `ui-core/src/transform_entry.rs`
   is at 548 non-test lines and is not split by this feature; and "one peer
   delete refuses a whole multi-object commit" joins the `drag_objects` note
   under "Canvas performance".

No question for the customer comes from the architecture.
