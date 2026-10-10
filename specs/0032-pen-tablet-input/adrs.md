# ADRs for "Pen tablet input"

Pressure arrives through the webview's Pointer Events, which ADR 0001
already provides. The fit uses `kurbo`, and the variable-width outline uses
`i_overlay` `=9.0.1`'s integer variable-width stroke (round joins and ends,
which is exactly criterion 8). **No new dependency, no new crate, no new
ADR, no `format_version` bump** (a pressure stroke is stored as an ordinary
closed path, spec Question 2, A), no trait, no generic. Reading pen axes
natively on Linux, if the demo shows WebKitGTK does not deliver pressure,
would need its own ADR (platform integration, possibly `unsafe`). It is
not part of this slice.

## Depends on

- [ADR 0001](../../docs/adr/0001-ui-framework-and-canvas-rendering.md): the
  editor core runs as wasm in the system webview (WebKitGTK on Linux,
  WebView2, WKWebView). Pointer Events are its input path on every target.
  The frontend renders, and the core decides.
- [ADR 0003 §1, §2, §3](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  curve fitting on `kurbo` and polygon work on `i_overlay`'s `i64` engine
  on the 0.001 mm grid, both in `curvyo-geometry-core` with an explicit
  tolerance.
- [ADR 0002 §5, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one stroke is one new path node and one commit, labelled `pencil_stroke`.
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md):
  samples, the in-flight preview and the Pencil settings are ephemeral. They
  are never in the document, the log or a file (criteria 14, 16).
- [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) and
  [`specs/0020-undo-redo/adrs.md`](../0020-undo-redo/adrs.md): the label
  `pencil_stroke` goes into the name table `history_names.rs` as "Pencil
  stroke" (criterion 13). The guard test there enforces it.
- [`specs/0038-path-offset/adrs.md`](../0038-path-offset/adrs.md): the
  integer `i_overlay` API (not the float adapters) and the arc-step rule for
  a 0.01 mm chord tolerance.

## Feature-local decisions

- **2026-10-10: the input record.** `curvyo-ui-core` gets one plain struct,
  `PointerSample { position, pointer_kind: Mouse | Pen | Touch, pressure,
  tilt_x_deg, tilt_y_deg, eraser, time_ms }`, in `pointer_sample.rs`. Tilt
  and eraser are carried and read only by the readout (criteria 1, 15).
  Only the Pencil and the readout use it. The existing `pointer_down`,
  `pointer_move` and `pointer_up` of the other tools keep their signatures,
  so every mouse test passes unedited (criterion 1). A pen hover (`buttons`
  0) goes through the existing move path (criterion 5).
- **2026-10-10: coalesced samples cross the wasm boundary once per event.**
  The frontend reads `getCoalescedEvents()` when it exists (criterion 2) and
  passes the batch as one flat `Float64Array` (6 values per sample) plus the
  kind and the buttons to one call, `pencil_samples` in `wasm_pencil.rs`.
  There is one wasm call per DOM event, not one per sample. No pointer id
  or device name crosses the boundary (criterion 16).
- **2026-10-10: crate placement.**
  - `curvyo-ui-core`: `pencil_tool.rs` (the stroke state machine, the
    "pressure present" rule of criterion 3, touch ignored, the minimum
    length) and `pencil_pressure.rs` (smoothing, the three curves, the
    width mapping, criterion 9). Both are pure.
  - `curvyo-geometry-core`: `freehand_fit.rs` and `variable_outline.rs`
    (below).
  - `curvyo-editor-wasm`: `Tool::Pencil`, `session/pencil.rs` (session
    settings, preview, the commit) and `wasm_pencil.rs`.
  - `curvyo-render-core`: the blue preview (a line, or the outline).
  - `frontend`: the event wiring, the Pencil bar and the readout, per the UX
    notes.
- **2026-10-10: the fit, `freehand_fit.rs`.** The samples become a polyline
  and are simplified with `kurbo::simplify_bezpath` at 0.08 mm accuracy, a
  margin under criterion 7's 0.1 mm. A test then measures every sample's
  distance to the result with `nearest_point_on_segment` instead of trusting
  the accuracy argument. `kurbo` itself warns that noisy input fits poorly.
  If a real tablet stroke breaks the node budget at the demo, Schneider's
  fit goes into the same module. That is our own code and still needs no
  dependency.
- **2026-10-10: the variable-width outline, `variable_outline.rs`.** It
  flattens the fitted centre line within 0.01 mm and gives each vertex the
  width interpolated by arc length from the samples. It snaps to the grid
  and calls `IntVariableStrokeOffset::variable_stroke` with
  `MathMode::Integer` and the arc step for 0.01 mm. Then it runs `cleanup`
  and `canonical_millimetres` from the boolean pipeline. The error budget
  is 0.01 (centre line) + 0.01 (arcs) = 0.02 mm (criterion 8). Preview: the
  same function on the raw sample polyline, unfitted. Fit (0.08) + outline
  (0.02) stays under criterion 11's 0.15 mm between preview and object.
- **2026-10-10: the stored result.** It is written with the existing
  path-creation command: an open path for a stroke without pressure, or a
  closed path (or compound path, if the stroke crosses itself and leaves a
  hole) for one with pressure. Fill takes the new-path stroke colour and
  stroke is None. These are existing kinds and keys, so there is no bump.
- **2026-10-10: budgets.** Criterion 10 (2,000 samples, under 600 nodes, under
  200 ms) and criterion 11 (a preview per frame up to 2,000 samples) are
  release-build tests in the `boolean-budgets` CI job. If the preview misses
  the frame, it decimates the raw samples to 0.05 mm spacing before the
  outline, and nothing else changes.

## Flagged

- Demo item 3 of the spec (no pressure on Linux): reading GDK pen axes in
  the desktop shell would be a new ADR and its own slice. It is not
  decided here. The default stays "a pen draws like a mouse on that
  platform".
- Spec Question 2, B (an editable width profile) would be a document-model
  and format change. It belongs with `0033-stroke-brushes` and its ADR, not
  here.
