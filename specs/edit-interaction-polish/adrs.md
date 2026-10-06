# ADRs for "Edit interaction polish: star and polygon angle, typed skew and move, copy and one-axis move, Escape, Split selection, keyboard shortcuts"

This feature adds **one `Document` operation** (`duplicate_objects`, Part C)
and nothing else to `vecmanf-document-core`'s public surface except one
read-only method (`ObjectSnapshot::orientation`, Part A). Every value it
writes goes to a register that already exists, with its existing meaning.
**No stored field, no new key, no `format_version` change (it stays at
`main`'s 5), no new crate, no new external dependency, no ADR amendment.**
Part A's format question (criterion 2) is answered "no change", see decision 1;
because of that nothing here is a `needs-customer` ADR (`CLAUDE.md` §3: no
platform, UI framework, document model, persistence, plugin, license or
account decision).

Reference state: `main` after PR #35, with `unified-object-editing` (#38,
both of its PRs) assumed merged. Everything below was checked against the code
on `origin/story/unified-object-editing` (read-only). Facts used: the creation
tools are creation-only and a committed create-drag selects the new shape and
switches to the Select tool (needed by Part A and criterion 44);
`SelectTool::live_edit` and `LiveEdit` exist (needed by Part C);
`select_tool.rs` is 380 non-test lines, `session/mod.rs` 506, `wasm_api.rs`
760, `transform_entry.rs` 516, `render-core/select_decoration.rs` 416.

**Buildability.** Parts A, B, D, E and F are buildable as written, with the
readings below. Part C is buildable; criterion 41's second half (the commit
finishing before the next frame) is not guaranteed, flag 3. Eight spec points
are contradictory or unbuildable as written and carry a default under "Flagged
to the PO" (criteria 21, 41, 59, 64 and four smaller ones).

## Depends on

- [ADR 0001 §1, §3, §4](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  every rule (the key table, the Escape order, the press classification, the
  axis choice, the typed-entry arithmetic, the dash fitting) is plain Rust in
  `ui-core`, `render-core` or `Session`; the frontend forwards scalars and owns
  only what the DOM alone knows (focus, IME, chip text, badge position).
- [ADR 0001 §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  new wasm calls carry strings and scalars only.
- [ADR 0002 §3](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  offsets are `Vec2`, positions `Point`, angles `Angle`; degrees and typed
  millimetres exist only at the entry and readout edges. Every comparison below
  names its tolerance.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one drag, one typed entry, one copy is one commit; a zero offset, an unedited
  chip or an equal result writes nothing.
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md): the
  modifier state, the axis lock, the badges, the chips, the Escape step and the
  key gate are ephemeral. A copy is an ordinary new tree node (no
  cross-object link), so it cannot conflict with an edit of its original.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md): no
  new key and no new meaning of a key (decision 1 is the proof for Part A).
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md): every piece
  fits an existing crate over an existing edge.
- [`specs/unified-object-editing/adrs.md`](../unified-object-editing/adrs.md):
  `LiveEdit` (blue-new, black-old), `EditHandle`, the one hit rule,
  `OpenEntry`, `ParamEntry`, `session/select_view.rs`. This feature adds a
  field to `LiveEdit` and two variants to `OpenEntry`; it adds no second drag,
  hit or preview rule.
- [`specs/object-transform-refinements/adrs.md`](../object-transform-refinements/adrs.md):
  "one resolving function per gesture shared by preview, release and typed
  entry" (`skew_by_angle`, `rotate_by`, `resize_by_local_delta`),
  `DragOrigin` and the 3 px dead zone, `Session::modifiers_changed`, the angle
  snap table.
- [`specs/shape-creation-from-center/adrs.md`](../shape-creation-from-center/adrs.md):
  `Modifiers { shift, ctrl }` in `ui-core::modifiers`. Whichever of the two
  features is built first introduces it; the other uses it. It is used here for
  the Polygon/Star create-drag and for the Select tool's move.
- [`specs/advanced-selection/adrs.md`](../advanced-selection/adrs.md): its press
  state machine and its `alt` field arrive after this feature (decision 7).
- [`specs/0006-path-merge-split-and-node-types/adrs.md`](../0006-path-merge-split-and-node-types/adrs.md):
  `split_at_anchor` and "moved anchors keep their `AnchorId`s"; criterion 15 is
  superseded by criteria 50 to 52 here.

## Feature-local decisions

### 1. Polygon and star angle: keep `StarFrame.angle`, define the shown angle as a sum (criterion 2)

- **2026-10-06: the shown angle is `StarFrame.angle + rotation`, normalized to
  `(-180°, 180°]`. No stored field changes.** The outline is already drawn
  this way (`outline_of_rotated` turns the frame's own first-vertex angle by
  `rotation` about the frame centre), so the sum is the real orientation by
  definition, today, in every existing file. What is wrong today is that four
  readers of the angle read `rotation` alone.
  - New read-only `ObjectSnapshot::orientation() -> Angle` in
    `vecmanf-document-core` next to `rotation()`: a polygon or star returns
    `(frame.angle + rotation).normalized()`, every other kind returns
    `rotation`. It sits in document-core because "the angle of the first
    vertex in document space" is the model's own meaning of two registers, the
    same arithmetic `outline_of_rotated` composes. Additive, pure, wasm32-safe.
  - The four readers move to it: the rotate readout in
    `session/select_view.rs` (`format_degrees(live.rotation()...)`),
    `TransformEntry::for_rotate`'s prefill, `start_value(FieldAxis::Angle)` and
    the typed target in `TransformEntry::apply` (`transform_entry.rs`, three
    sites). A typed `A` becomes `change = normalized(A - orientation)`, applied
    through the existing `rotate_by`, so `rotation` advances by `change` and the
    shown angle lands on `A` (criterion 7, "0 puts the first vertex right").
  - **Ctrl rotate of a polygon or star snaps the shown angle (absolute).**
    `rotate_delta_angle` keeps its relative rule for every other kind; a new
    `rotate_delta_for(object, pivot, down_at, current, ctrl)` in
    `transform_math.rs` (the one caller of `rotate_delta_angle` besides tests)
    returns, for a polygon or star under Ctrl,
    `normalized(snap_angle(orientation + raw) - orientation)`. Preview and
    release both call it through `TransformDrag::resolve`, so criterion 14 of
    the refinements holds unchanged. Worked: orientation 45°, raw 10°, snapped
    stop 60°, delta 15°; orientation 78.7°, raw 1°, stop 75°.
  - **Create-drag.** The Polygon/Star tool gets `pointer_move(point,
    modifiers)` and `pointer_up(document, point, modifiers)` (the
    `shape-creation-from-center` pattern: one private
    `created_frame(center, current, modifiers)` is the only computation, the
    preview passes the stored pointer and modifiers, the release passes the
    release event's). Ctrl replaces the angle of `center -> current` with
    `snap_angle(raw)` and keeps the radius `|AB|`, then
    `StarFrame { angle: snapped, radius, center }`. `LiveShape`'s readout gains
    the angle through `format_degrees(orientation)` (one decimal at most).
    `rotation` is still written 0 by `create_polygon`/`create_star`; nothing
    new is stored.
- **Options considered.**
  - **(b) Move the created angle into `rotation` and keep `StarFrame.angle` at
    0, `format_version` 5 to 6.** Rejected. Old files cannot be read as one
    representation: a file with a star at frame angle 78.7° and rotation 30° has
    to stay as it is, so a reader still needs the sum, which is option (a). The
    migration would be a rewrite of every polygon and star register on open,
    and every peer that opens an old file would write it, which makes
    concurrent first opens conflict (`star_frame` is one LWW register, a
    rewrite by peer A can beat a concurrent resize by peer B,
    `specs/0005-object-transform/adrs.md`'s merge table). It also makes
    `format_version` 6 older-build-hostile for no stored gain. The one thing (b)
    would give, a selection box that turns with a freshly created shape, is not
    asked for: criterion 1 only asks that rotating by Δ turns the box by Δ.
  - **(c) Only new shapes store their angle in `rotation` (frame angle 0), no
    version change.** Rejected: old and new files then hold the same shape in
    two representations for good, so the sum is needed anyway.
  - **(a) chosen.** One definition, no migration, no write, no version.
- **File and merge semantics.** Old files open with the same shapes and show
  `frame.angle + rotation` (criterion 2, required). New files are byte-for-byte
  the kind main already writes, so a build from before this change opens them
  with the same shapes (criterion 2, Should, holds for free). Concurrency is as
  today: `rotation` and `star_frame` are separate LWW registers; a centre
  rotate writes `rotation` only, a resize writes the frame, a rotate about a
  non-centre pivot writes both. The shown angle after a merge is the sum of
  whichever value each register converged to, which is the angle the outline is
  drawn with on every peer.
- **Known limit.** The oriented box of a polygon or star turns by `rotation`
  alone, so a shape created at 78.7° has an axis-aligned circumscribed box
  while its shown angle is 78.7°. That is today's look and satisfies
  criterion 1. `docs/technical-debt.md` ("Rotation is a stored angle") gets a
  dated note: the primitives rework can fold `StarFrame.angle` into `rotation`
  with a real migration if the box turning with the shape is ever wanted.
- **Tests (first).** `orientation()` over both registers and the wrap at ±180°
  (a first vertex left shows 180, not -180); a version-5 fixture with a star at
  frame angle 78.7° / rotation 0 and one at frame angle 10° / rotation 30°
  (shown 40°) opens unchanged and re-saves identical bytes of the shape
  registers; create-drag golden: A = (100, 50), B = (110, 48), Ctrl: first
  vertex (109.85, 47.36) to 0.01 mm, radius 10.20 mm, angle -15°; drag, release
  and typed entry agree (`rotate_by` equality within 1e-9 mm and 1e-12 rad) for
  both kinds with and without Ctrl and Shift; "typed 0 puts the first vertex at
  `center + (R, 0)`"; the axis-edge table of criterion 7 (hexagon 0°, pentagon
  0°, square 45°, octagon 22.5°, dodecagon 15°) against the real outline
  vertices.

### 2. Duplicating objects: `Document::duplicate_objects` (Part C, criteria 34, 35, 36)

- **2026-10-06: one operation, one commit, in `vecmanf-document-core/src/
  objects.rs`.**

  ```rust
  pub struct CopySource { pub id: NodeId, pub anchor_ids: Vec<AnchorId> }
  pub fn duplicate_objects(&self, sources: &[CopySource], offset: Vec2)
      -> Result<Vec<NodeId>, ObjectEditError>
  ```

  It returns the new ids in source order. `ObjectEditError` gains
  `AnchorIds` (a path source whose `anchor_ids` length differs from its anchor
  count). Steps, in one commit labelled `duplicate_objects`:
  1. Resolve every source before the first write (one stale id refuses the
     whole call, as `translate_objects` does); `dedup` the source list.
  2. For each source `tree.create(Root)`, copy **every key of the source's meta
     map** to the new node, and for a path rebuild the `anchors` movable list
     entry by entry with the caller's ids (each other key of each anchor map
     copied as is).
  3. Move the node directly above its own source:
     `tree.enable_fractional_index(0)` then `tree.mov_after(new, source)`, the
     way `split_open_path` places its new object. A selection of several keeps
     its relative order by construction (A, B gives A, A', B, B').
  4. Translate the copy with the existing `translate_primitive_meta` /
     `translate_path_meta`, the functions `translate_objects` and
     `ObjectSnapshot::translated` already share, so a copy's position cannot
     differ from the blue outline that previewed it. A zero offset is the
     caller's refusal (criterion 36), not this function's.
- **Why a key-by-key copy and not "read the snapshot, call `create_*`".** The
  snapshot types do not carry every register today (a primitive's `fill` is
  always `None`; `0007` adds dash, join, cap, gradient; `rectangle-corner-radii`
  makes the radius four registers; `ellipse-arcs-and-shaping` adds three). A
  copy built from snapshots silently drops each of them the day it is added; a
  copy of the meta map takes every register, including keys a newer build wrote
  that this build does not know. Criterion 34's "every stored field of its
  original" is therefore structural, not a checklist. If the pinned Loro cannot
  iterate a map's keys, the fallback is the known key set (`KEY_*` constants of
  `path_codec` and `shape_codec`); the test below catches a key added later
  without the copier knowing it.
- **`AnchorId`s are fresh.** The caller mints them; `document-core` never mints
  (`CLAUDE.md` §6, `AnchorIdMinter`'s own rule). Two reasons, both observed in
  the code: `NodeSelection::contains_node(anchor)` and `Join` identify a node by
  its `AnchorId` alone, and `join_two_objects` copies anchors between paths
  keeping their ids, so a copy that shares ids with its original would put two
  equal ids into one path the first time a maker joined a path to its copy. The
  caller reads each path source's snapshot, mints `anchors.len()` ids from the
  session's `AnchorIdMinter` and passes them in `CopySource`. A primitive passes
  an empty vector. This is the one place the minter enters Select-tool code:
  `SelectTool::pointer_up` and the typed-move commit take `&mut AnchorIdMinter`
  (the session owns one and already borrows it disjointly from `select`).
- **Selection after.** The tool sets the selection to exactly the returned ids
  (criterion 35) and stays Select. The commit lives in
  `transform_commit.rs` next to `commit_gesture` as
  `commit_move(document, ids, offset, copy, minter) -> Option<Vec<NodeId>>`:
  `copy == false` calls `translate_objects`, `copy == true` calls
  `duplicate_objects`. A drag, a Ctrl release and the typed move (criterion 23)
  all call it, so there is one commit path per outcome.
- **Undo.** `undo-redo` does not exist; the single commit with a distinct label
  is the one undo step it will need. Nothing else is added for it.
- **Concurrency.** The copy is a new tree node with no reference to its
  original. A peer's concurrent edit of the original does not touch it. A peer
  who concurrently deleted the original makes `mov_after` refuse on the
  receiving side of a stale local read: the call returns `NoSuchObject` and
  writes nothing (the same stale-id rule as every command). A concurrent
  `duplicate_objects` on both peers creates two copies, as two clicks would.
- **Cost.** The Select tool tessellates only the moved objects for the preview,
  as `LiveEdit` does now. The commit writes about 6 keys per anchor; see flag 3
  for criterion 41's commit clause.
- **Tests (first).** For one path with curves and kinds, one rectangle with a
  radius and a rotation, one ellipse, one polygon, one star: the copy's
  `export_json` equals the original's except `NodeId`, anchor ids and the
  offset (this catches a register the copier forgot, now and later);
  `ObjectSnapshot::translated(offset)` of the original equals the copy's
  snapshot within 1e-9 mm (preview equals commit); z-order A, A', B, B' for a
  selection given in order B, A; anchor ids are all new and unique across the
  document and a `join_endpoints` of a path's end to its copy's end succeeds
  with distinct ids; `anchor_ids` of the wrong length refuses and writes
  nothing; a stale source refuses and writes nothing; one commit; a saved and
  reopened document holds the copies. Merge test: peer A duplicates, peer B
  moves the original, both converge to a moved original and a copy at A's
  offset from the original's old position.

### 3. Typed skew and typed move (Part B)

- **2026-10-06: two new entry types, two new modules, `transform_entry.rs`
  does not grow.** It is 516 non-test lines already. `OpenEntry`
  (`select_tool/entry.rs`) gets `Skew(SkewEntry)` and `Move(MoveEntry)`
  beside `Transform` and `Param`: four concrete types use the enum, each with
  its own fields and commit.
  - **`ui-core/src/skew_entry.rs`** (about 150 lines). One field, the fixed
    suffix "°" in the chip, accessible name "Skew angle x" (top, bottom) or
    "Skew angle y" (left, right), prefill "0". It reuses `EntryField`,
    `EntryOutcome`, `parse_entry_number(text, true)` and `format_degrees`. Its
    `resolve(text)` is: untouched text or an angle within 1e-12 rad of 0 is
    `Ok(None)`; not a number is `NotANumber`; `|α| >= 90°` is the new
    `InvalidReason::SkewRange`; otherwise `skew_by_angle(start, start_box,
    side, shift, α)`, **the same function `TransformDrag::resolve` calls for a
    drag** (criterion 10's "one resolving function"), and if that returned the
    start for a non-zero α the result was refused by `sane_or`'s coordinate
    limit and the entry reports the new `InvalidReason::TooLarge`. `commit`
    goes through `commit_gesture(document, EditHandle::Skew(side), ...)`, the
    drag's own commit (anchors and handle vectors, `rotation` and style
    untouched). The fixed line and the Shift pivot are fixed when the entry
    opens (`skew_frame(box, side, shift).fixed_point`) and shown by the pivot
    marker: `live_pivot` gains a `Skew` arm. Shift at the second press is read
    from the double-click modifiers; the key route passes `shift = false`
    (criterion 58). A flat path (zero extent across the skew axis) has no
    distance to scale by: its field is marked non-editable, as the existing
    zero-extent resize field is, rather than reporting a silent "no change".
  - **`ui-core/src/move_entry.rs`** (about 200 lines). State: the object, its
    start snapshot, the start tight bounds, `[EntryField; 2]` for X and Y with
    two prefill sets (Relative "0", Absolute the current top-left X and Y,
    `format_mm`), and nothing about the mode: **the mode and the Copy check
    arrive with the commit call**, so the DOM owns them and the rule exists
    once. `resolve(texts, absolute, copy) -> Result<Option<MoveResult>,
    (usize, InvalidReason)>` where a field whose text equals the prefill of the
    mode in force is "untouched, no change on that axis" (criterion 19, the
    same untouched-text rule as every entry, so no per-field edited flags cross
    the wasm boundary; the chip's own re-render of an untouched field on a mode
    switch is DOM state and affects no rule). Offset: Relative `(X, Y)` with an
    untouched field 0; Absolute `(X - x0, Y - y0)` per touched axis, 0 for an
    untouched one, `(x0, y0)` the top-left of the start tight bounds. A
    non-finite value, or a result beyond `MAX_COORDINATE_MM` (1e7 mm, the
    existing constant in `transform_commit.rs`), is `NotANumber` (criterion 22
    "Enter a number"). An offset within 1e-9 mm of zero is `Ok(None)`
    (criterion 25). `commit` calls `commit_move` (decision 2) with the typed
    offset and `copy`. The entry holds the start snapshot like every entry, so
    one that outlived its object (a collaborator's edit, an undo) writes
    nothing.
  - **Absolute reference: tight bounds.** New `object_outline_bounds(&ObjectSnapshot)
    -> (Point, Point)` in `ui-core/src/object_bounds.rs`: a path's existing
    tight `path_bounds` (curve extrema through `segment_bounds`), and for a
    primitive the same union over `outline_of_rotated(shape, rotation)` (the
    outline that is drawn, so a rotated rectangle and a star are measured on
    what is on screen, and `ellipse-arcs-and-shaping`'s arcs follow with no
    change here). `path_bounds` generalises over "anchors with handles" so
    paths and outlines share it (two concrete users). Stroke width is not
    included. This is **not** `object_bounds` (today a primitive's unrotated
    frame box): flag 1.
- **Where the chips open: named handle positions, hidden or not.** An entry
  stores its anchor position in document space, computed when it opens from the
  layout's own position functions (`resize_handle_local_position`,
  `rotate_handle_local_position`, `skew_handle_local_position`, the box centre
  for the move), not looked up in the drawn handle set. Today
  `session/transform_entry.rs` finds the handle by searching
  `SelectTool::transform_handles` and returns `None` when the handle is not
  drawn, which would make R, S and K fail on a 40 px box. The new pure function
  `entry_anchor(box, handle, tolerances) -> Point` goes in
  `select_tool/handles.rs` (158 lines; `transform_handle_layout.rs` is at 480
  non-test lines and stays out of it). `TransformEntry`, `ParamEntry`,
  `SkewEntry` and `MoveEntry` all carry the point; `EntryView` reads it. The
  handle's dragging look still comes from `entry_handle()` and simply does not
  show where the handle is not drawn.
- **Double-click on the centre handle (criteria 15, 16).** `hover_handle_at`
  already returns `Move` only where the centre handle is drawn and only after
  every other handle failed (it is "hit last, inside its hover region"), so the
  hit area needs no new rule. Two changes in `select_tool`: a press that lands
  on it records `last_press_handle = Some(EditHandle::Move)` (today a centre
  press records `None`), and `double_click` tries
  `handle_at(...)` first, then `hover_handle_at(...) == Move` with the first
  press on `Move`, then the old "inside the sole box" handoff. Where the centre
  handle is not drawn the old rule stands (criterion 17). `SelectDoubleClickOutcome::Ignored`
  is deleted (no handle is ignored any more); `Hit` and `EditHint` are
  unchanged for the rest of the box and the outline.
- **Keys M, R, S, K, Shift+K open the same entries.** One ui-core entry point,
  `SelectTool::open_entry_for_key(objects, selection, key: EntryKey, tolerances)
  -> Result<(), KeyEntryRefusal>` with `EntryKey { Move, Angle, Size, SkewX,
  SkewY }` and `KeyEntryRefusal { NothingSelected, SeveralSelected,
  SkewNeedsPath }` (the three hint texts of criterion 59). It builds the entry
  with the same constructors as a double-click, with `shift = false` and no
  Ctrl (R: `for_rotate(.., Ne, false)`, S: `for_resize(.., Se, (false, false),
  modes)`, K: Skew Top, Shift+K: Skew Right, M: the centre). The entry
  is therefore identical for the two routes except for the Shift pivot, which
  only the double-click reads. The size entry for a polygon or star is the
  outer radius, as today.
- **wasm surface, without touching `wasm_api.rs`.**
  - The skew entry reuses `TransformEntryView`: `kind` is `"skew"`, one field,
    the accessible name above. `commit_transform_entry` and
    `cancel_transform_entry` are unchanged; the outcome strings gain
    `invalid-skew-range` and `invalid-too-large`.
  - The move chip has its own view and calls, in a new
    `vecmanf-editor-wasm/src/wasm_move_entry.rs` (a second `#[wasm_bindgen]
    impl WasmSession` block, the pattern of `wasm_select_bar.rs`): `move_entry()
    -> Option<MoveEntryView>` (handle position, centre, the two prefill sets,
    `copy_preset`), `commit_move_entry(x, y, absolute, copy) -> String`.
    Session glue in a new `session/move_entry.rs`. Frontend: one new
    `MoveEntryChip.tsx` (two fields, the `role="switch"` mode control, the Copy
    check, its Tab order), and the skew chip is the existing angle chip with a
    different name.
- **Hints (criterion 24).** The handle hint strings for the centre, skew,
  rotate and resize handles gain their "or M / K / R / S" suffix in the one
  place that builds them (`Session::handle_hint`, `select_view.rs`).
- **Tests (first).** Skew: for a path at rotation 0 and at 30°, with and
  without Shift, every top, bottom, left and right handle: the entry result
  equals `TransformDrag::resolve` with a pointer chosen for the same α
  (1e-9 mm); the 20 x 10 example (+10 mm on the top edge, 0 on the bottom, +5
  halfway); skew then negated skew restores every anchor and handle vector
  (1e-9 mm, coordinates to 1 m); 90° and -90° are `SkewRange`; 89.9° on a path
  reaching 1e7 mm is `TooLarge`; "0" and untouched writes nothing and closes; a
  flat path's field is read-only; K opens the same chip as the double-click for
  a 20 px path. Move: Relative (5, -3) from top-left (10, 20) gives (15, 17);
  Absolute (100, 50) on a 30 x 40 rectangle at origin (10, 20) gives origin
  (100, 50); a 30° rotated rectangle and a star: Absolute puts the **tight**
  top-left at (X, Y) to 1e-9 mm; one untouched field means no change on its
  axis in both modes; the same text is read in the mode current at Enter;
  `NaN`, empty, "1,2,3", 1e8 are `NotANumber` and write nothing; the drag of the
  same offset and the typed Relative move leave identical registers; typed move
  of a path writes only anchor points; the chip opens for every kind and for a
  15 px object by M; a collaborator's delete makes Enter write nothing.

### 4. The keyboard gate and the key table (Part F, criteria 54 to 62)

- **2026-10-06: `Session::key_down` decides; the frontend forwards and reports
  what only the DOM knows.** Rule location options: (A) the rule in the
  frontend's `onKeyDown`, as today; rejected: it is not reachable by a Rust
  test, which is exactly how "R while dragging switches tool" shipped. (B) The
  rule in `Session`, the frontend passing the key, the modifiers and four DOM
  facts; chosen. The rule needs the tool, the drag, the Pen path, the open chip
  and the selection kind, all of which `Session` owns, and "which tool is
  active" is `Session`'s documented job (ADR 0001 §3); the entry arithmetic it
  triggers stays in `ui-core` (decision 3). Placing it in `ui-core` was
  rejected: `Tool` lives in `editor-wasm`, and moving it for one `match` is
  churn.
- **Module and shape.** `vecmanf-editor-wasm/src/session/keys.rs` (new, with
  `escape` and `delete_selected` moved into it so `session/mod.rs`, at 506
  non-test lines, shrinks):

  ```rust
  pub struct KeyInput<'a> { key: &'a str, shift: bool, ctrl: bool, alt: bool,
                            repeat: bool, dom_blocked: bool }
  pub enum KeyOutcome { Ignored, ToolChanged, EntryOpened, Deleted,
                        Escape(EscapeStep), PenFinished, Hint(KeyHint) }
  pub fn key_down(&mut self, input: KeyInput<'_>) -> KeyOutcome
  ```

  `dom_blocked` is one boolean the frontend computes: focus is in a text field,
  select, button, `role="switch"` or contenteditable element, or an IME
  composition is running (`event.isComposing`), or Space is held. All of it is
  DOM knowledge. Everything else in criterion 55 is read from `Session`: a
  Select drag, a Node tool node or handle drag, a create-drag, a Pen path in
  progress, a pan (`is_panning`), an open entry (`select.has_entry()`), and
  `repeat`. The pure core is one function
  `decide(input, state: KeyState) -> KeyAction` over a plain `KeyState { tool,
  operation_in_flight, pen_open, entry_open, selection: None | One(Kind) |
  Many }` in the same file, so the table is a property test, not an integration
  test. `wasm_api.rs` gets no line: the call is in a new `wasm_keys.rs` (a second
  `impl` block) taking `(key: &str, shift, ctrl, alt, repeat, dom_blocked)` and
  returning a string code (`"ignored"`, `"tool"`, `"entry"`, `"deleted"`,
  `"escape-drag"`, `"escape-state"`, `"escape-tool"`, `"escape-none"`,
  `"hint-select-first"`, `"hint-select-one"`, `"hint-path-only"`, `"pen"`).
  The frontend calls `syncFromSession()` for every result except `"ignored"`
  and calls `preventDefault()` for every result except `"ignored"`, so Ctrl+R
  and an app-level Ctrl+S keep their page and menu behaviour (criterion 55).
- **The gate (criterion 55), in this order.** `repeat` (Escape: acts on the
  first press only, criterion 60; every other key: nothing); `ctrl` or `alt`
  (the frontend sends `ctrl = ctrlKey || metaKey`); `shift` unless the key is
  `*` or K; `dom_blocked`; `operation_in_flight`; `pen_open` (for tool letters,
  entry keys and Delete; Enter, Escape and Space are not gated by it); then
  the binding. Letters are matched case-insensitively (Caps Lock changes
  nothing); Shift is read from the flag, so Shift+K is `K` with `shift`.
  `Escape`, Enter (finishes the Pen path, as today) and Space are never gated by
  the list. Escape against `dom_blocked`: the frontend does not forward an
  Escape typed into a control (the control handles it, criterion 42 step 1),
  so `dom_blocked` is ignored for Escape here; an entry chip that holds focus
  closes itself and returns focus to the canvas, and `Session` also closes an
  open entry on Escape (step 1) so a test without a DOM sees the same order.
- **Bindings (criterion 54).** B, N, E, `*`: switch tool, in every state.
  R: Select tool with at least one object selected opens the angle entry
  (`EntryKey::Angle`); every other state selects the Rectangle tool. S: the
  same with `Size` and the Select tool. M, K, Shift+K: `open_entry_for_key`;
  a refusal is `Hint(...)`; outside the Select tool M and K are
  `Hint(SelectFirst)` and change no tool. Delete and Backspace: as today
  (Select deletes the selection, Node the nodes, Pen nothing) behind the same
  gate. The three hint texts are frontend strings; the frontend shows the
  one-line hint chip for 2 s (a new generic transient message next to
  `EditHintChip`, which already has the timer and placement).
- **Rail tooltips (criterion 62)** need one more fact in the frontend's state:
  whether the Select tool has an object selected. `syncFromSession` already
  reads the Select bar's state; `select_bar_state` gains no field, a new
  `WasmSession::selection_count()` in `wasm_keys.rs` returns it.
- **Tests (first), in `session/keys.rs`.** The table of criterion 54 as one
  parameterised test over (tool, selection kind, key, shift); every gate
  condition on its own and in pairs for every tool letter, M, K, Delete (no
  tool change, no deletion, no entry, the document and the drag identical to the
  same drag without the key: one commit equal to the commit without the
  key); Ctrl+R, Cmd+R, Alt+E, Ctrl+S change no tool and return `Ignored`;
  key repeat of B, R, Delete and Escape; three-node Pen path: B, N, E, R, S, `*`,
  Delete do nothing, Enter finishes it, a first Escape discards it, a second
  Escape switches to Select; R, 0, Enter through the Session turns a star shown
  at -15° to 0° (the first tip straight right); a "rectangle drawn then R"
  opens the angle chip and Escape, R selects the Rectangle tool.

- **2026-10-06 (implementer, PR 1): `Hint(SelectFirst)` was deliberately not
  added.** Until PR 3 and PR 4 give M and K a chip, those keys are ignored
  (`KeyOutcome::Ignored`, no `preventDefault`): a "Select an object first"
  hint for a key that does nothing even with an object selected would mislead.
  `KeyHint` has only `SelectOne` in PR 1; PR 3 adds `SelectFirst` and
  `PathOnly` with the keys that need them.

### 5. Move modifiers, copy, axis lock and their indicators (Part C)

- **2026-10-06: the move becomes its own small state machine in a new child
  module `select_tool/move_drag.rs`; `select_tool.rs` shrinks.** Today
  `SelectDrag::Moving { origin, from_center }` and the move arms of
  `live_offset`, `live_edit` and `pointer_up` are spread over `select_tool.rs`
  and `preview.rs`.
  ```rust
  pub(super) struct MoveDrag { origin: DragOrigin, from_center: bool,
      pending_toggle: Option<NodeId>, joins: Option<NodeId>, last_axis: Option<Axis> }
  pub(super) struct MoveResolution { offset: Vec2, copy: bool, axis: Option<Axis> }
  impl MoveDrag { pub(super) fn resolve(&self, current: Point, m: Modifiers) -> MoveResolution }
  ```
  `resolve` is a pure function of the press point, the current point, the
  modifiers and `last_axis` (the one piece of event history, used only for an
  exact tie). It is the **one resolving function**: `live_edit` calls it for the
  blue outline and the readout, `pointer_up` calls it with the release
  point and the release modifiers, and the typed move reuses only the commit
  (decision 2), as the refinements entry reuses `resolve` for its drag. Rules
  (criteria 28 to 32): `D` is the displacement from the press point; with Shift
  the axis is `x` if `|Dx| > |Dy|`, `y` if `|Dy| > |Dx|`, `last_axis` (x if
  none) on a tie within 1e-12 mm, re-chosen on every call, no latch; the offset
  is `(Dx, 0)` or `(0, Dy)`; `copy` is `ctrl` at the call; Shift released gives
  `D` again with nothing accumulated. `pointer_moved(point, modifiers)` (it now
  takes `Modifiers`, and `&mut ObjectSelection` for the join below) updates
  `last_axis` from `resolve`. A zero resolved offset within 1e-9 mm writes
  nothing, with and without copy (criteria 31, 36), in the same guard that
  already closes the zero-offset move hole.
- **The press is classified once.** New `select_tool/press.rs` holds
  `classify_press(objects, selection, point, tolerances, shift) -> PressTarget`
  with `PressTarget { Handle(EditHandle), CentreHandle, InsideSelectedBox,
  Object(NodeId), Empty }`, extracted from `pointer_down` (the order is the one
  `pointer_down` has today: handle, inside the sole selected box when Shift is
  up, outline, empty). `pointer_down` becomes `classify_press` plus a `match`.
  The plus badge's "wherever a press with Ctrl would start a move" (criterion
  33) calls the same function on the hover position, so the badge cannot
  disagree with the press; a test presses at a grid of points over a mixed
  scene and asserts `badge_shows == (pointer_down begins a Moving drag)` for
  Ctrl held. `advanced-selection` extends `PressTarget` with its marquee arms
  (criterion 37's "that meaning wins") instead of threading them through
  `pointer_down` again.
- **Shift at the press (criterion 29).** `pointer_down` with Shift on an object
  no longer toggles. It records `pending_toggle = Some(object)` and, when the
  object is not selected, `joins = Some(object)`. `pointer_moved` adds
  `joins` to the selection in the frame the drag leaves the dead zone;
  `pointer_up` toggles `pending_toggle` only when the drag never left it. A
  Shift press on an already selected object keeps the selection for a drag and
  removes the object on a click, as before. A press on the centre handle with
  Shift or Ctrl never records either (criterion 38).
- **`LiveEdit` gains `copy: bool`.** Copy mode makes `objects` the translated
  copies and `live_objects_in` (`session/select_view.rs`, the one substitution
  point) skips the substitution when `copy` is set, so the selection boxes and
  the handles stay on the originals while the blue outline travels alone
  (criterion 33). In a move the boxes follow as they do now. The centre handle
  keeps its dragging look at the original in copy mode.
- **Indicators and who owns them (criteria 26, 27, 33).**

  | Indicator | Owner | Where the rule is |
  |---|---|---|
  | Blue outline of the copy or moved result | `render-core` `build_live_edit_preview` (unchanged) | `LiveEdit` |
  | Origin axes, two lines, locked one `--axis-guide`, the other `--axis-guide-idle` | `render-core` | new `MoveAxes { horizontal: (Point, Point), vertical: (Point, Point), locked: Axis }` in `SelectDecorationInput`; the endpoints are the full viewport at the start centre, computed by `Session` (render-core has no canvas size); two theme tokens; drawn after the artwork and before the blue outline, composed in `session/draw.rs` |
  | Readout "Δ 12.5, −3.0 mm" with " Copy" | `Session::select_live_readout` (`select_view.rs`) | text from `MoveResolution` (after the lock); real minus U+2212 in the readout only |
  | Plus badge, lock badge | DOM, `MoveBadges.tsx` | `Session::move_indicators() -> MoveIndicators { copy_badge: bool, lock: Option<Axis> }`, a pure read of the cached modifiers, the hover position, `classify_press` and the drag; the frontend re-reads it from `applyModifiers` and every pointer event, so the badges follow a key event with the pointer at rest (criterion 26). Lock badge not before the press. |

  `render-core` receives plain `Point`s, a `bool` and an `Axis` and never a
  `ui-core` type (ADR 0011 §3). The DOM badges carry no rule.
- **Release, Escape and the Ctrl that is still held.** The release uses the
  release event's modifiers. Escape cancels the drag (decision 6) and writes
  nothing; `move_indicators` is recomputed from the cached Ctrl, so a held
  Ctrl keeps showing the plus badge (criterion 26).
- **Frontend, the one change in input plumbing.** `pointer_down` stays
  `(x, y, shift)`. `pointer_hover`/`pointer_up` already carry shift and ctrl.
  Nothing new crosses the boundary except `move_indicators()`, in a new
  `wasm_move.rs`, and `pointer_cancelled()` (decision 6).
- **Sizes.** `select_tool.rs` ends below 400 non-test lines (the move arms and
  `pointer_down`'s classification move out); `move_drag.rs` about 160;
  `press.rs` about 140. `docs/technical-debt.md` keeps `advanced-selection` as
  the slice that finishes the job (its marquee/lasso/cycle states go in sibling
  modules the same way).
- **Tests (first).** `MoveDrag::resolve` table: criterion 30's whole worked
  example, step by step, with the final (31, 80) and (0, 80); the tie keeps the
  previous axis; Shift at press with a drag of 4 px: locked from the first
  frame; Shift released: the free offset is `D` exactly; Shift click without
  movement toggles on release; Shift drag of an unselected object joins it at the
  frame it leaves the dead zone; a Shift press on the centre handle never
  toggles; Ctrl at press and at release; Ctrl pressed and released mid-drag
  (preview and release agree with the release state); Shift+Ctrl: copy along
  the axis; a drag back to the start and a locked axis ending at 0 make no copy
  and write nothing; Escape then Ctrl still shows the badge; the 200-object
  copy preview benchmark (`#[ignore]`, release, budget 8 ms of CPU per
  `draw_list()` as in `unified-object-editing`) and the commit measured, flag 3;
  the badge/press property above; the origin-axes input is absent before the
  dead zone is left and gone when Shift is released.

### 6. The Escape cascade, the Node tool and Split selection (Part D)

- **2026-10-06: `Session::escape() -> EscapeStep`, one step per call, written
  once.** `EscapeStep { ClosedEntry, CancelledDrag, ClearedState, LeftTool,
  Nothing }` (strings `"escape-*"` over wasm). It returns a value now (the 44
  existing callers ignore it; nothing else about the call changes). Order,
  exactly criterion 42:
  1. `select.has_entry()` (an open chip): close it, return `ClosedEntry`. The
     chip's own DOM Escape and this are idempotent. A bar field's Escape is
     the DOM's (it never reaches the canvas).
  2. A drag in flight in the active tool: cancel it, return `CancelledDrag`.
     Per tool: Select `SelectTool::escape`, Node `NodeTool::cancel_drag`
     (new), a creation tool `*.escape()` returning whether a drag existed, Pen
     below.
  3. The tool's own state: Pen `PenTool::escape` (discards a path with at least
     one node, also mid handle drag: steps 2 and 3 in one, as today), Node
     `NodeTool::clear_selection` (new, returns whether anything was
     selected), Select `ObjectSelection::clear` (returns whether anything was
     selected); a creation tool has none. A successful step 3 returns
     `ClearedState`.
  4. Otherwise a non-Select tool becomes the Select tool through `set_tool`
     (`LeftTool`; the object selection is exactly what it was, criterion 47),
     and the Select tool returns `Nothing`.
  `NodeTool::escape` (clears the selection and the drag in one call) is split
  into the two new functions and deleted. A hint chip or tooltip is not a step.
- **"A drag in flight" and "the button is still held" are two facts
  (criterion 49).** The first is each tool's own state, read exactly. For the
  second `Session` gets `button_down: bool`, set in `pointer_down`, cleared in
  `pointer_up` and in a new `pointer_cancelled()` (the frontend calls it on
  `pointercancel` and window blur, next to `applyModifiers(false, false)`).
  While `button_down` is true, steps 3 and 4 are skipped: a second Escape with
  the button still held "does nothing more". The gate of decision 4 uses the
  tools' own state, not `button_down`, so a lost `pointerup` can at worst make
  Escape do less, never block letters.
- **Pen.** `pen.escape()` already does steps 2 and 3 together; with no path in
  progress the cascade reaches step 4 and switches to Select, as criterion 43
  says, and Enter and the double-click still finish a path.
- **Key repeat.** `key_down` returns `Ignored` for an Escape with `repeat`
  (criterion 60), so a held Escape in the Node tool clears the selection once
  and does not go on to leave the tool.
- **Split selects one node (criteria 50 to 52).** `NodeTool::split_selected`
  selects only `second` (`split_at_anchor` returns the pair `(first, second)`
  and `second` is the fresh id the caller minted: the new first node of the
  second object for an open path, the new first node for a closed one; a test
  per case pins that it is the copy that keeps the outgoing handle). Three
  supporting changes:
  - **Hit-test tie goes to the selected node.** `hit_test_node` takes the
    `NodeSelection` and, for two nodes within 1e-9 mm of the same distance,
    prefers a selected one over the first in document order. Without it the
    pair at the shared position would hit the unselected first one and a drag
    would move the wrong end. `hit_test`'s existing "a handle wins an exact
    tie" is unchanged.
  - **Draw order.** `render-core/src/decorations.rs` draws selected node glyphs
    after the unselected ones, so the selected fill is not hidden by a
    coincident unselected glyph.
  - **Toolbar.** `NodeToolbarState` needs no change: with one selected end node
    `can_join` and `can_split` are already false (`0006` criterion 12), and
    the node-type buttons act on one node. Re-Join of a just-split pair is
    Undo's job (criterion 52).
- **Tests that are rewritten, not deleted, with an equal or stronger assertion:**
  `vecmanf-ui-core/src/node_tool.rs`: `split_selected_on_an_interior_node_selects_both_new_objects_nodes`
  and `split_selected_on_a_closed_path_node_opens_it_selecting_both_ends` become
  "selects exactly the second node, the first is not selected, a press at the
  shared point hits the selected one, a drag moves one end only";
  `split_then_rejoin_restores_one_object` selects the second node explicitly
  before the Join (it used the old both-selected state);
  `escape_mid_drag_cancels_it` and `escape_clears_a_present_selection_and_is_a_no_op_otherwise`
  become the two-step Node behaviour (the drag is cancelled and the selection
  is kept, then the selection clears, then nothing is left to clear).
  `vecmanf-editor-wasm/src/session/mod.rs`: `split_selected_on_an_interior_node_through_the_session`.
  `vecmanf-editor-wasm/tests/acceptance_0006*.rs`: the criterion-15 assertions.
  New: the full cascade as one test per tool (Select, Pen, Node, Rectangle,
  Ellipse, Polygon/Star), including "Escape with the button held, then Escape"
  and a held key repeat; the Select tool's Escape clears the selection and
  hides the per-kind bar groups; after Escape from a creation tool the Select
  tool has nothing selected and the next Escape does nothing; Pen: three nodes,
  Escape discards, Escape switches.

### 7. Dashed selection box, pixel alignment, skew guide (Part E, criteria 63 to 68)

- **2026-10-06: render-core only, plus the device pixel ratio as a scalar.**
  - **Module.** `render-core/src/select_decoration.rs` (416 non-test lines) has
    the box, the handle glyphs, the skew guide and the radius guide in one file,
    and the dash fitting, the snap and the axes would take it past 500. A pure-move
    prelude (task 1 of its PR, no behaviour change) takes the box drawing
    (`build`, `SelectionBox`, the quad outline calls) into a new
    `select_box.rs`, whose one-sentence doc is "draws the selection and hover
    boxes". The new code goes there, so the file that keeps the handles does not
    grow.
  - **Dash fitting (criteria 63, 64).** For each box edge of screen length `L`
    pixels, `fit_dashes(L) -> Option<(n, dash, gap)>`: an edge under 10 px is
    solid (`None`); otherwise the smallest `n` dashes with `4n + g(n-1) = L` for
    a gap `g` in 2 to 4 is used, which is symmetric about the edge's centre and
    has a dash at both ends, so every corner is closed. Where no `n` fits with a
    dash of exactly 4 (edge lengths strictly between 12 and 16 px and between 20
    and 22 px: `[6n-2, 8n-4]` leaves those holes), the gap is 2 and the dash
    flexes to `(L - 2(n-1)) / n` (between 2.7 and 4 px); flag 2. The pattern is
    laid out in the box's own (possibly rotated) frame from the edge's first
    corner, a pure function of the corners, so panning, zooming and a move shift
    it rigidly and nothing crawls. Dashes are built with the existing
    `dashed_guide` pieces (short solid quads); a dash is not tessellated across
    a corner. `dashed_guide`'s fixed-pitch loop is not reused for the box
    because it cannot centre the pattern.
  - **Pixel alignment (criteria 65, 66).** An axis-aligned box or hover box
    edge snaps to the device pixel grid: in screen space,
    `snap(x) = (round(x * r - c) + c) / r` with `r` the device pixel ratio and
    `c = 0.5` when `round(r)` (the line width in device pixels, at least 1) is
    odd and `0` when even. A rotated box is not snapped. `render-core` still
    never sees the canvas size or the ratio today (`gpu.rs` alone knows
    `devicePixelRatio`), so `SelectDecorationInput` gains
    `device_pixel_ratio: f64` (default 1.0, non-finite or non-positive reads as
    1.0), set by `Session` from a new `set_device_pixel_ratio` that the
    existing wasm `resize(w, h, dpr)` wrapper calls on the session as well as on
    the GPU. It enters render-core as a scalar like the view scale and keeps the
    draw list a pure function of its inputs. The snap moves the line by at most
    half a device pixel; a tolerance test asserts at most 0.5 device pixels of
    displacement and a coverage test asserts one full device row or column at
    ratios 1, 1.25, 1.5, 2 and 3 (criterion 65's 3.7:1 is the full-coverage
    number, measured by the `ux-engineer` in the PR review, not by a unit
    test).
  - **Hover box (criterion 66)** stays solid at `--accent-hover` and uses the
    same snap. **Box over artwork (criterion 67)** needs no code: the box is
    already drawn above the artwork and below the handles.
  - **Skew guide (criterion 68).** The guide moves from `GUIDE_DASH_PX` 4 /
    `GUIDE_GAP_PX` 3 to new `SKEW_GUIDE_DASH_PX` 2 / `SKEW_GUIDE_GAP_PX` 2, full
    `--accent`, same extent and lifetime. The radius guide and the lasso keep
    4 / 3 (the existing constants stay for them). Test update:
    `the_skew_guide_draws_as_dashes` (its "ten dashes at 70 px" arithmetic
    changes to eighteen at 2 / 2, asserted exactly).
  - **Tests (first).** `fit_dashes` over every length from 10 to 400 px in 0.25
    steps (a dash at both ends, the pattern symmetric, gap within 2 to 4 except
    in the two flagged bands, dash never over 4 and never under 2.5); edges under
    10 px solid; the dash phase is the same after translating the whole box by
    any offset and zooming (rigid); a rotated box's dashes follow its edges;
    each selected object of a multi-selection has its own dashed box; the
    marquee box stays solid; the draw list of two equal inputs is equal.

- **2026-10-06 (PR 2 build, reviewed): four refinements of this decision.**
  - **`fit_dashes` picks the gap nearest 3**, not the smallest count. Among the
    counts with an exact fit (dash 4, gap 2 to 4) the one whose gap is nearest
    the nominal 3 is used. The smallest count is the largest gap and would draw
    nearly every edge as 4 on / 4 off, not the customer's V1 (4 on / 3 off).
    The two flex bands are unchanged (gap 2, dash about 2.7 to 4).
  - **End dashes overshoot the corner by half a line width** (first and last
    dash of every edge of a selection box), so the two edges cover the whole
    corner pixel and the corner is closed, not notched. The 20 percent hover
    box is not extended: it would blend the corner pixel twice.
  - **An edge longer than 50,000 screen pixels is drawn solid.** render-core
    has no viewport, so this bounds the draw list at absurd zoom, where the
    edge is far off screen anyway.
  - **A sub-pixel pan re-fits a pixel-snapped box by at most one pixel of
    length.** Criteria 63 (rigid pattern) and 65 (pixel-snapped line) pull
    apart here: a snapped box changes its pixel length by one when its true
    edges straddle a pixel boundary differently, and the dashes re-fit (for
    example gap 2.929 vs 2.857 px over 15 dashes). Accepted. A whole-pixel
    translation and a zoom of a box that keeps its snapped size leave the
    pattern identical; the pattern is a function of the snapped pixel length.
    Tested by `ac63_ac65_a_snapped_box_keeps_its_pattern_rigid_and_refits_only_when_its_pixel_length_changes`.

- **2026-10-07 (PR 2 UX review): the skew guide takes the box's snap, and the
  box yields the fixed edge.** `TransformDecorationInput` gains
  `device_pixel_ratio` (the guide snaps with the box's own `snap_to_device`,
  full accent, whole device pixels; a rotated guide stays 1 px anti-aliased).
  `SelectDecorationInput` gains `skew_guide`: a selected box's edge that lies on
  it (both ends within 1 px of the guide line) draws no dashes inside the
  guide's extent, so the guide's 2 / 2 reads alone and does not fill the box's
  4 / 3 gaps. A Shift guide through the centre cuts no edge. Both are scalars
  and points; no new crate, trait or dependency.

### 8. Sequencing, the PR split, what each PR deletes and rewrites

- **2026-10-06: four PRs, in this order, each a `story/` PR that needs
  customer acceptance (`CLAUDE.md` §9).** The story is done when all four are
  in. Cut lines are chosen so each PR leaves the app consistent: no PR leaves a
  key that opens a chip that does not exist, a badge without its move, or a spec
  criterion half true.

  **PR 1 `story/edit-polish-orientation-escape-keys`: Part A, Part D, the gate
  and the R and S entries of Part F (criteria 1 to 8, 42 to 52, 54 to 55 except
  M and K, 57, 60 to 62).** Document-core: `ObjectSnapshot::orientation`. UI-core:
  `rotate_delta_for`, the four `rotation()` readers, the Polygon/Star
  `Modifiers` create-drag (and `modifiers.rs` if not present), `NodeTool::
  cancel_drag`/`clear_selection`, the Split change, the hit-test tie. Editor-wasm:
  `session/keys.rs`, `wasm_keys.rs`, `escape() -> EscapeStep`, `button_down`,
  `pointer_cancelled`. Render-core: node glyph draw order. Frontend: `onKeyDown`
  becomes a forwarder (the letter `switch` is deleted), Escape handling, hint
  chip, rail tooltips. Tasks, in order:
  1. Pure-move prelude: `escape` and `delete_selected` from `session/mod.rs`
     to `session/keys.rs` (mod.rs falls below 500).
  2. `orientation()` and the four readers, with the fixture tests (criteria 1,
     2, 6, 7).
  3. Polygon/Star create-drag with Ctrl and its readout (3, 4, 5, 8).
  4. `decide` and the gate with its table tests, then `key_down` for B, N, E,
     `*`, R, S, Delete, Enter (54, 55, 57, 60, 61); the frontend forwarder.
     M and K return `Hint(SelectFirst)` until PR 3 gives them a chip: they are
     listed as not yet bound in the PR description.
  5. The Escape cascade (42 to 49); `NodeTool` split into two functions.
  6. Split selects one node, the tie, the draw order (50 to 52).
  7. Rail tooltips and `selection_count` (62), `docs/design-system.md` key table
     (`ux-engineer`), demo.

  **Deletes or rewrites:** the frontend letter `switch` in `onKeyDown`;
  `NodeTool::escape`; the tests listed in decision 6; the tests that read the
  rotation of a polygon or star in the rotate readout, the angle entry or the
  Ctrl rotate snap (`vecmanf-ui-core/tests/acceptance_object_transform_refinements.rs`
  and `acceptance_otr_tester.rs`, the polygon/star rotate cases) are rewritten
  to `orientation()`; `0006` criterion 15's tests.

  **PR 2 `story/edit-polish-dashed-box`: Part E (criteria 63 to 68).**
  Render-core and two wasm lines (`set_device_pixel_ratio`). Independent of PR 1
  and PR 3 except for the shared `select_decoration.rs`, so it can run in
  parallel with PR 1 (they share only `session/draw.rs`'s input assembly); the
  lead decides. Tasks: pure-move prelude (`select_box.rs`), `fit_dashes`, the
  snap, the ratio, the skew guide constants, the `ux-engineer` review of the
  rotated skew guide at 1x (criterion 68's note), `docs/design-system.md` rows.
  **Deletes or rewrites:** `the_skew_guide_draws_as_dashes`; any decoration test
  asserting a solid box outline's triangle count (rewritten to the dashed
  count).

  **PR 3 `story/edit-polish-typed-skew-move`: Part B, and M and K of Part F
  (criteria 9 to 25 except the Copy check of 23, and 56, 58, 59).** UI-core:
  `skew_entry.rs`, `move_entry.rs`, `object_outline_bounds`, `entry_anchor`,
  `open_entry_for_key`, the centre-handle double-click, `last_press_handle`
  for the centre. Editor-wasm: `session/move_entry.rs`, `wasm_move_entry.rs`,
  `TransformEntryView` kind `"skew"`. Frontend: `MoveEntryChip.tsx`, skew chip,
  hint strings. Tasks: (1) `object_outline_bounds`; (2) `entry_anchor` and moving
  the three existing entries to a stored anchor (so R and S work on a 15 px
  box); (3) `SkewEntry`; (4) `MoveEntry` with `commit_move` writing the
  translate path only (`copy` is accepted by `commit_move` and refused by the
  chip until PR 4: no Copy check is drawn); (5) double-click routing and
  `Ignored` deleted; (6) M, K, Shift+K in `key_down`; (7) hints, review, demo.
  **Deletes or rewrites:** `SelectDoubleClickOutcome::Ignored` and its assertion
  at `acceptance_object_transform_refinements.rs:625` (skew handle: now an entry);
  every test that double-clicks the centre handle and expects the handoff or the
  hint chip (the centre-handle clause of `unified-object-editing` criteria 31
  to 33: `acceptance_otr_dblclick.rs`, `acceptance_otr_tester.rs`,
  `acceptance_unified_editing*.rs` where they press the centre of a small box
  and a large one: the large-box cases now open the move chip, the small-box
  cases keep the old assertion); `refinements` criterion 54's hint strings.

  **PR 4 `story/edit-polish-move-copy-lock`: Part C and the Copy check of
  criterion 23 (criteria 23, 26 to 41).** Document-core: `duplicate_objects`.
  UI-core: `move_drag.rs`, `press.rs`, `LiveEdit.copy`, `pointer_moved(point,
  modifiers)`, the Shift join and toggle. Render-core: `MoveAxes`, two tokens.
  Editor-wasm: `move_indicators`, the readout, `live_objects_in`'s copy rule,
  `commit_move` with a minter. Frontend: `MoveBadges.tsx`, the Copy check in
  the move chip. Tasks: (1) `duplicate_objects` with its tests; (2)
  `classify_press` as a pure extraction (behaviour unchanged, the old
  `pointer_down` tests green); (3) `MoveDrag` and `resolve`, moving the move
  arms out of `select_tool.rs`/`preview.rs` (pure move first, then the
  modifiers); (4) Shift at press, join and release toggle; (5) copy commit and
  selection; (6) `LiveEdit.copy`, decorations on the originals; (7) axes,
  readout, `move_indicators`, badges; (8) the Copy check and Ctrl at the second
  press of the typed move; (9) benchmark, review, demo. **Deletes or
  rewrites:** `live_offset` (folded into `MoveDrag::resolve`); the Shift-press
  toggle-at-press assertions of slice 4 (`acceptance_0004.rs`,
  `vecmanf-ui-core/tests/acceptance_0004.rs`: the toggle now happens on
  release); `shape-creation-from-center`'s and `unified-object-editing`'s tests
  that press with Shift inside a selected box stay as they are (criterion 29
  keeps that case).
- **Order against the other Ready specs.** This feature goes **before
  `advanced-selection`**, directly after `unified-object-editing`. The
  recommended order is: `unified-object-editing` (#38), then this feature (PR 1, PR 2 in parallel or
  first-come, PR 3, PR 4) then `shape-creation-from-center`, `rectangle-corner-radii`,
  `0007-stroke-and-fill-styling`, `advanced-selection`, `ellipse-arcs-and-shaping`.
  Reasons: it is the customer's own testing round of #38 and fixes bugs in
  accepted behaviour (the letters firing mid-drag, Escape); it rewrites the
  press and move dispatch that `advanced-selection` would otherwise have to
  thread its marquee and Alt states through twice, and it leaves `PressTarget`
  and `MoveDrag` as the places `advanced-selection` extends; `duplicate_objects`
  copies every register by key, so none of `rectangle-corner-radii` (four radius
  registers, a `format_version` bump), `0007` (style registers) or
  `ellipse-arcs-and-shaping` has to touch it, and Part A needs no version, so
  the next `format_version` is still `rectangle-corner-radii`'s to take. If
  `shape-creation-from-center` has not landed, PR 1 introduces
  `Modifiers { shift, ctrl }` and it uses it; if `advanced-selection` is
  somehow built first, PR 4 re-bases onto its `Pending*` states and
  `Modifiers { alt }`, and criterion 37's conditional becomes unconditional.
- **Parallelism.** At most two implementers (`CLAUDE.md` §4). PR 2 may run
  beside PR 1. PR 3 and PR 4 must be serial (both edit `select_tool/entry.rs`,
  `session/select_view.rs`, `transform_commit.rs`). No PR runs beside another
  spec.
- **Superseded criteria elsewhere, exactly** (the lead updates the owning specs
  on acceptance, as the specification's "Changes to accepted behaviour" lists
  them): `0006` criterion 15; `object-transform-refinements` criterion 49, the
  centre-handle sentence of criterion 3, the "typed skew" out-of-scope line and
  criterion 56's `4 / 3`; `unified-object-editing` criteria 31, 32 and the
  centre clause of 33; `shape-creation-from-center` criterion 17 for Ctrl only;
  `advanced-selection`'s "needs Esc first" note becomes true.

### 9. Crates, dependencies, purity, size limits

- **No new crate, no new dependency.** `vecmanf-document-core` gains
  `duplicate_objects`, `CopySource` and `ObjectSnapshot::orientation`;
  `vecmanf-ui-core` gains `skew_entry.rs`, `move_entry.rs`,
  `select_tool/move_drag.rs`, `select_tool/press.rs`, one function in
  `object_bounds.rs`, `entry_anchor` in `select_tool/handles.rs`,
  `rotate_delta_for` in `transform_math.rs`; `vecmanf-render-core` gains
  `select_box.rs` and `MoveAxes`; `vecmanf-editor-wasm` gains `session/keys.rs`,
  `session/move_entry.rs`, `wasm_keys.rs`, `wasm_move_entry.rs`, `wasm_move.rs`.
  Each new module's doc comment is one sentence without "and" (`CLAUDE.md` §5).
  No new trait; the one new enum-with-two-more-variants is `OpenEntry`, an
  `enum` with an exhaustive `match`, not a trait.
- **Purity and wasm32.** Everything new is in the `*-core` crates or in
  `editor-wasm`'s plain-Rust `session` module: no filesystem, network, clock,
  thread or UI. The only clock use is the `#[ignore]` benchmark's. Each core
  crate keeps `#![forbid(unsafe_code)]` and the wasm32 build in the gate.
  `document-core` still needs its JavaScript host on wasm32
  (`docs/technical-debt.md`), unchanged. Dependency direction is unchanged:
  `render-core` takes `Point`, `bool`, `Axis` and `f64`, never a `ui-core` type.
- **Units as types.** Offsets `Vec2`, positions `Point`, angles `Angle`, the
  create radius `Length`. Millimetres and degrees appear only in the typed
  fields and readout strings, converted at the entry's edge as the refinements
  entry does. New named tolerances: the zero-offset epsilon (1e-9 mm, the
  existing `MOVE_EQUAL_EPSILON_MM`), the axis tie epsilon (1e-12 mm), the
  skew-zero epsilon (1e-12 rad, the existing angle epsilon), the pixel-snap
  half pixel.
- **Size limits (`CLAUDE.md` §5, `docs/technical-debt.md`).** `select_tool.rs`
  ends below 400 non-test lines (380 today, the move and press classification
  move out). `session/mod.rs` falls below 500 (`escape` and `delete_selected`
  move to `keys.rs`). `transform_entry.rs` (516) and `wasm_api.rs` (760) do not
  grow: this feature adds no line to either. Their existing debt remains
  `advanced-selection`'s to split, as `technical-debt.md` already records; **this
  feature does not have to split `wasm_api.rs` first**, because every new wasm
  call lives in a sibling `impl` block and every signature change is
  line-neutral. Rule for the PRs: the architect checks `wc -l` of these two
  files in each PR and refuses a net increase. `session/select_view.rs`
  (about 430 non-test lines) takes the readout and the `live_objects_in` rule
  and ends near 480. `render-core/select_decoration.rs` shrinks by the box move.
- **Quality gate additions.** `cargo deny` unchanged (no dependency). The
  frontend has no test runner (`docs/technical-debt.md`, "The quality gate
  covers only half the product"), so everything testable above is in Rust; the
  chips, the badges and the forwarder are covered by the `ux-engineer` review
  and the demo.

## Flagged to the PO

1. **Criterion 21 reads the selection's box wrongly.** "tight bounds ... as the
   selection's plain box" is not what `object_bounds` returns for a primitive
   (today its unrotated frame box: for a rotated rectangle it ignores the
   rotation, for a star or polygon it is the circumscribed square, larger than
   the outline). The criterion's own examples need the tight outline of the
   drawn, rotated shape, which is a new function (`object_outline_bounds`,
   decision 3). **Default: build that function; reword "as the selection's
   plain box" to "of the drawn outline". A star's absolute X, Y is therefore
   not the top-left of its selection box.**
2. **Criterion 64 cannot hold literally for two edge lengths.** With a dash of
   exactly 4 px, a gap of 2 to 4 px and a dash at both ends, an edge of `n`
   dashes must have length between `6n - 2` and `8n - 4` px, which leaves edges
   strictly between 12 and 16 px and between 20 and 22 px with no pattern.
   **Default: in those two bands the gap is 2 px and the dash flexes between
   about 2.7 and 4 px; every corner stays closed.** If the dash must stay 4 px
   the alternative is an open corner on those edges, which the customer's
   "closed corners" finding rejects.
3. **Criterion 41's second half (the commit finishes before the next frame) is
   not guaranteed.** A copy of 100 paths with 50 anchors and 100 rectangles
   writes about 30,000 keys and 200 tree nodes (a move writes 5,000 anchor
   points); in WebAssembly that is likely tens of milliseconds, not under 16.7.
   The preview half is as buildable as the move's today. **Default: the preview
   keeps the 8 ms budget of `unified-object-editing`; the commit has no frame
   budget; the implementer measures it in release and the number goes into the
   PR and the demo. The PO may reword the second clause to "the maker sees no
   gap longer than 100 ms between release and the copies on screen".**
4. **Criterion 59 contradicts the refinements for a side rotate handle.** It
   hides the Shift-revealed side rotate handles "while any chip is open", but
   a double-click on a side rotate handle (with Shift) opens its angle chip and
   `object-transform-refinements`' `side_rotate_revealed` keeps that handle
   drawn, in its dragging look, until the chip closes. **Default: while a chip
   is open only the side rotate handles other than the open chip's own are
   hidden**; the key routes never open a chip on a side handle, so the stated
   purpose (no chip on a handle that is not there) holds.
5. **Criterion 23 does not say what is selected after a typed copy.**
   Criterion 35 says the selection is the copies after a copy commit.
   **Default: the same for the typed copy**; the PO may add the sentence to 23.
6. **Criterion 6 and 27 use two minus signs.** The create readout reuses
   `format_degrees` (ASCII "-15°", which is also the angle entry's prefill and
   what `parse_entry_number` accepts); the move readout uses U+2212 per 27.
   **Default: keep both as written, and make `parse_entry_number` also accept
   U+2212 so a value copied from the move readout parses.** Not a conflict, a
   consistency note.
7. **The "Escape" criteria name states that do not exist yet.** Criteria 42
   (marquee, lasso) and 37 (the marquee arm) describe `advanced-selection`.
   Until it ships they are conditionals of that spec; PR 1 and PR 4 implement
   everything that exists and `advanced-selection` honours them when built
   (the ADR note is in decision 8).
8. **Part A leaves a freshly created shape's selection box axis-aligned**
   (decision 1, "Known limit"). Criterion 1 is satisfied. If the customer
   expects the box to turn with the shape at creation, that is the rejected
   option (b) (a file-format change with a migration) and a separate story.
