# ADRs for "Shape creation from the center: Shift and Shift+Ctrl while drawing"

This feature changes how the rectangle and ellipse tools turn a press point
and a pointer position into a box. It writes the same fields through the
same `Document::create_rect` / `create_ellipse` commands as today.
**No new crate, no new external dependency, no new `vecmanf-geometry-core`
or `vecmanf-document-core` function, no ADR amendment, no `format_version`
change, no `wasm_api.rs` change and no frontend change.**

Nothing in the specification is unbuildable. Two criteria need a small
reading, under "Flagged to the lead".

## Depends on

- [ADR 0001 §1, §4](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the Shift/Ctrl-to-box rule is interaction logic and lives in
  `vecmanf-ui-core`; `editor-wasm` and the frontend only forward key state as
  scalars.
- [ADR 0002 §3](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  the box is in document millimetres; tests compare with an explicit
  tolerance.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one create-drag is one commit on release; A = E writes nothing.
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): the
  modifier state and the create-drag are ephemeral; nothing records which
  modifiers were used (criterion 13).
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  no new key and no new meaning of a key, so no version bump.
- [`specs/0003-primitive-shapes/adrs.md`](../0003-primitive-shapes/adrs.md):
  the primitive schema, `RectBounds::from_corners` / `EllipseFrame::from_corners`,
  and its flag 2 (a one-axis drag creating a zero-height rectangle is kept as
  specified).
- [`specs/object-transform-refinements/adrs.md`](../object-transform-refinements/adrs.md):
  `Session::modifiers_changed(shift, ctrl)` from window-level key events,
  followed by the frontend re-sending `pointer_hover` at the last pointer
  position, which is how criterion 8 reaches the shape tools; the
  `session/mod.rs` split. *2026-10-06 (architect):* as built (PR #35),
  `modifiers_changed` only caches the state and does not re-run the hover
  itself, so a `Session`-level test of criterion 8 calls `pointer_hover`
  with the new modifiers.
- [`specs/advanced-selection/adrs.md`](../advanced-selection/adrs.md):
  its planned `Modifiers { shift, ctrl, alt }` in `vecmanf-ui-core`, which
  this feature introduces first (below).

## Feature-local decisions

- **2026-10-06: one resolving function, in `shape_tool_common.rs`.**
  `create_drag_box(a, b, modifiers) -> Option<CreateDragBox>` returns the
  two opposite corners of the box and the readout anchor E, or `None` when
  E = A (criterion 12). It calls `constrained_endpoint` unchanged for E
  (Ctrl), then returns corners `(A, E)` without Shift and `(2A − E, E)` with
  Shift. Each tool maps the corners with the existing `from_corners`.
  - Options: (A) a sibling function that composes `constrained_endpoint`;
    chosen. (B) A `centered` flag on `constrained_endpoint`; rejected: the
    function would return a box instead of an endpoint, its name would be
    wrong, and the readout anchor E would still need a second call.
  - Inside each tool, one private `created_shape(down_at, point, modifiers)
    -> Option<(Shape, Point)>` calls `create_drag_box` and is the only
    computation: `live_shape` passes the stored `current` and modifiers,
    `pointer_up` passes the release point and the release modifiers
    (criteria 9, 10). Today `live_shape` and `pointer_up` each repeat the
    `if constrain` branch; that duplication goes.
  - `EllipseFrame::from_corners` computes the center as a midpoint, so in
    centered mode it equals A to within a few ulps, not bit-exactly. That is
    inside any tolerance a test uses; the spec's worked examples are exact.
  - Polygon/star is not touched by this feature (criterion 17); its
    create-drag already takes `Modifiers` (see the 2026-10-07 note).

- **2026-10-06: Shift reaches the tools as `Modifiers`, not a third
  parameter.** New `vecmanf-ui-core/src/modifiers.rs`: `pub struct Modifiers
  { pub shift: bool, pub ctrl: bool }` (`Copy`, `Default`). The rectangle
  and ellipse `pointer_move(point, modifiers)` and `pointer_up(document,
  point, modifiers)` take it in place of `constrain: bool`, and
  `RectDrag::Creating` / `EllipseDrag::Creating` store it in place of
  `constrain`.
  - Options: (A) the struct; chosen. It is the type `advanced-selection`
    already decided on for the Select tool, introduced here by its first
    user; `advanced-selection` adds `alt` to it (dated note in its
    `adrs.md`). Slice 5 avoided bool-heavy signatures for the same
    reason (`StrokeScaling`, `ResizeOptions`). (B) `pointer_move(point,
    shift, ctrl)`; rejected: two adjacent bools next to the old `constrain`
    name, call sites like `(p, true, false)`, and `advanced-selection` would
    rewrite the same signatures again. (C) A four-variant `CreateMode` enum
    chosen in `Session`; rejected: it moves the key-to-meaning mapping out of
    `ui-core` and encodes two independent flags as their product.
  - `Session` builds `Modifiers` from the bools it already receives in
    `pointer_hover` and `pointer_up` and passes it to `shape_pointer_move` /
    `shape_pointer_up`. The Select tool keeps its bools until
    `advanced-selection`; the shape-tool `pointer_down` keeps its `shift`
    (selection toggle, criterion 16, unchanged).
  - **No wasm or frontend change.** `WasmSession::pointer_hover` and
    `pointer_up` already carry `shift` and `ctrl || meta`. Criterion 8 (a key
    with the pointer held still) works through the existing re-sent hover:
    today the canvas-focused `refreshModifiers`, after
    `object-transform-refinements` its window-level `modifiers_changed`. Both
    end in `shape_pointer_move`.

- **2026-10-06: the readout needs no code.** `Session::live_readout` formats
  the `LiveShape::Creating(shape, anchor)` it gets: full width and height for
  a rectangle, rx and ry for an ellipse, anchored at E. In centered mode the
  shape is the full shape, so criterion 11 holds by construction. Add tests
  only.

- **2026-10-06: zero-extent drags are not changed here, and not by a
  `fix/`.** Criterion 12 keeps today's behaviour: only E = A is refused; a
  one-axis drag under Shift without Ctrl creates a zero-height (or
  zero-width) shape twice as long. This is accepted behaviour
  (`primitive-shapes` AC 1, its `adrs.md` flag 2), not a bug, so changing it
  is a behaviour change for the PO and customer (`CLAUDE.md` §3), together
  with the drag threshold this spec also leaves out. Downstream code already
  copes: `safe_factor` returns 1 for a zero-extent axis, the corner radius
  clamps to 0, and 0007 renders no gradient on a zero-size box.

- **2026-10-06: "new shape is not selected" is a separate `fix/`.**
  `primitive-shapes` AC 1, 7 and 11/12 say the created shape becomes the
  selected object. `Session::shape_pointer_up`
  (`vecmanf-editor-wasm/src/session/shapes.rs`) discards each tool's
  `Created(id)` outcome, so nothing selects it, for all three tools. Fix: on
  `Created(id)`, `self.selection.select_single(id)`, plus one
  `editor-wasm` test per tool. Branch `fix/select-created-shape`, standing
  merge permission. Independent of this feature; it touches one function, so
  it can land before or after it. This feature's tests must not assert
  either selection state after a create.

- **2026-10-06: tests.**
  - `ui-core` unit tests on `create_drag_box` with the spec's golden numbers,
    A = (100, 50): Shift to (130, 40), (70, 60), (70, 40), (130, 60) gives
    corners spanning (70, 40) to (130, 60) (criteria 1, 2, 5); Shift+Ctrl to
    (130, 40) and to (110, 20) gives (70, 20) to (130, 80) (criteria 3, 6);
    Ctrl alone to (130, 40) gives (100, 20) to (130, 50) and neither gives
    A to B (criteria 4, 7); B = A gives `None` under all four modifier
    states (criterion 12); a horizontal drag under Shift gives height 0.
    Anchor E asserted in each case. Tolerance 1e-9 mm (the spec's "document
    tolerance" is not a defined value; see flag 1).
  - One proptest: for finite A, B and any `Modifiers` with Shift, the box
    center is A within 1e-9 mm; and for random sequences of
    `pointer_move` calls with changing modifiers, the last `live_shape`
    equals what `pointer_up` at the same point and modifiers commits
    (criterion 9).
  - Acceptance tests through `Session` (tester): readout strings
    "60.0 × 20.0 mm" and "30.0 × 10.0 mm" (criterion 11); a modifier change
    with the pointer still (criterion 8, through `pointer_hover` or
    `modifiers_changed`, whichever exists); release modifiers differing from
    the last move (criterion 10); Escape then modifier changes (criterion
    14); pan mid-drag (criterion 15); Shift-press on an existing rectangle's
    outline toggles and creates nothing (criterion 16); polygon/star
    create-drag unchanged under Shift, Ctrl still snapping the angle
    (criterion 17; the handle-drag part is superseded, see the 2026-10-07
    note);
    saved `format_version` equals `CURRENT_FORMAT_VERSION` and the object
    has no new key (criterion 13).

- **2026-10-06: `format_version` unchanged, no new dependency.** The
  committed rectangle and ellipse are built by the same commands from the
  same field types; a reader of `main`'s version reads them exactly.
  `proptest` is already a dev-dependency of `vecmanf-ui-core`.

- **2026-10-06: sequencing.** After `object-transform-refinements` merges,
  as the spec says. Every candidate next feature (`0007`,
  `advanced-selection`) touches `vecmanf-ui-core` and `vecmanf-editor-wasm`,
  so none runs in parallel with this one (`CLAUDE.md` §4). Recommended
  slot: directly after `object-transform-refinements`, before `0007`: it is
  one small PR, the customer asked for it, and it shares no function with
  `0007`. If `0007` has already started, it goes after `0007`. It must land
  before `advanced-selection`, which then extends `Modifiers` instead of
  creating it. If it slips after `advanced-selection`, it uses that
  feature's `Modifiers` as is; nothing else changes.

- **2026-10-07 (architect, after `unified-object-editing` and
  `edit-interaction-polish`):** `Modifiers { shift, ctrl }` already exists on
  main (`vecmanf-ui-core/src/modifiers.rs`), so no new `modifiers.rs`;
  `Session` already builds `Modifiers` and passes it to
  `shape_pointer_move` / `shape_pointer_up`, and the polygon/star tool already
  takes it. This feature only changes the rectangle and ellipse tools to take
  `Modifiers` in place of `constrain: bool`. Criterion 16 and the first
  sentence of criterion 17 are superseded by `unified-object-editing`
  criterion 25. Polygon/star: Shift has no effect, Ctrl still snaps the
  create angle (`edit-interaction-polish` criterion 4).

## Flagged to the lead

1. **"The document's own geometric tolerance" (criteria preamble) is not a
   defined value.** No document-level tolerance exists. Default: tests use
   1e-9 mm, as `object-transform-refinements` does; the goldens are exact.
   The PO may reword to "1e-9 mm". No architectural impact.
2. **Criterion 8's "canvas keyboard focus"** is narrower than
   `object-transform-refinements`' window-level modifier forwarding.
   Not a conflict: whichever forwarding exists when this is built satisfies
   it. No change needed.
3. **Spec folder has no `NNNN` prefix** (`specs/README.md`), like
   `object-transform-refinements` and `advanced-selection`. The lead or PO
   assigns it when the feature enters `specs/index.md`; relative links in
   this file then need the new folder name.
