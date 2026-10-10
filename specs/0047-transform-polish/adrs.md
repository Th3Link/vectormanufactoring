# ADRs for "Transform polish: silent conversion, click-and-double-click sliders, a visible reference box for the typed move"

Written by the product owner at the lead's request to list what the feature touches; **the architect reviews it before the build starts** and replaces the "to decide" lines with decisions. **No new ADR, no new crate, no new dependency, no trait, no generic, and no `format_version` change are expected.** A path is an existing object kind; the box, the marks and the reference are view state. Reference state: `main` plus #80 (`story/multi-object-transform`); the picker comes from #78.

## Depends on

- [ADR 0001 §1, §3, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md): what a skew or stretch converts, the reference geometry (the nine points, the hit radius rule) and the slider value mapping stay in Rust (`curvyo-ui-core`); the box and marks are drawn by `render-core` from the decoration input; the DOM chip and the sliders only hold text, focus and gestures.
- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md): the per-node affine is still not built, so a skew has nowhere to live on a primitive and the conversion to a path is the consequence (P3, a stored matrix, stays deferred). [§9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md): the conversion and the transform are one commit.
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md): the box, the marks and the chosen reference are ephemeral and never written (criterion 49). A peer's concurrent edit of a primitive that this commit converts is the same case as a peer edit against "Object to path" today.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md): no new key and no new meaning of a key, so no bump (criterion 14).
- [ADR 0014](../../docs/adr/0014-history-undo-and-branches.md) (accepted 2026-10-10) and [`0020` adrs.md](../0020-undo-redo/adrs.md): a skew or stretch with a conversion is one step with the gesture's label (criterion 15).
- [`0019` adrs.md](../0019-multi-object-transform/adrs.md): decision 1 (`group_box.rs`, `group_transform.rs`, `Document::transform_objects`) is extended, and the sentence "Kinds are preserved; no conversion" of decision 2 is **superseded**: skew and non-uniform stretch of a uniform-only selection reached no per-object function before; now they do.
- [`0010` adrs.md](../0010-edit-interaction-polish/adrs.md) decision 3: the DOM chip holds the text, the mode and the focus; the reading of the numbers is `curvyo-ui-core::move_entry`. The reference is an extra input of that one function.
- [`0008`](../0008-object-transform-refinements/specification.md) decision P1 (customer, 2026-10-06; no ADR was written for it) is reversed by the customer's request of 2026-10-10.
- [`0044` adrs.md](../0044-editing-quick-wins/adrs.md): the nudge uses the same bounds (`object_outline_bounds`, `GroupSelection`); it does not change.

## Feature-local decisions

- 2026-10-10 (customer): a lone polygon or star that is stretched converts silently, like one in a multi-selection (spec question 2 B, criterion 12). The reference of the typed move has nine points; the Reference control is a 3 x 3 radio grid (question 8 B, criteria 39 to 41). Neither changes the file format: the conversion writes an existing kind, the reference is view state.
- 2026-10-10 (customer): 0047 and 0020 may be built in either order.

Points still open for the architect, each with the default the criteria assume:

1. **Where the conversion lives.** Default: the existing "Object to path" per-kind function is called from inside the skew and scale commit, so one `commit_with_label` writes the new path objects and the transformed anchors (criterion 4). Either `Document::transform_objects` takes a conversion step, or a sibling command composes the two writers. Either way: no second conversion function (criterion 3), all-or-nothing as `transform_objects` already is, and the commit label is the gesture's, never `convert_to_paths`.
2. **The decision "needs conversion".** Default: a pure function in `ui-core` next to `is_aligned_primitive` (`0019` decision 1): skew converts every primitive; a stretch converts polygon and star (also a lone one, criterion 12) and, in a multi-selection, the non-aligned rectangle and ellipse. A lone polygon or star then gets the single-object handle set of a rectangle (edge handles), so its kind-specific corner-only branch goes too. It replaces `is_uniform_only`; the group handle set no longer depends on kinds, so `HandleSpec` loses its kind-dependent branches (criteria 1, 7).
3. **The reference input.** Default: `MoveEntry::resolve` takes a `ReferencePoint` (one of nine: four corners, four edge midpoints, centre) and the bounds it already holds; `MoveEntry::new` computes the centre prefill. A small enum, two concrete uses at once (the chip and the click), so it is not a speculative abstraction. The click on a mark is a `PressTarget` that exists only while an Absolute chip is open (press order of `0019` criterion 43 gains one step before the handles, only then).
4. **Drawing the box and marks.** Default: the box is a `DecorationInput` item fed from the preview bounds the drag already computes (move) or from the entry's start bounds (chip); the marks are drawn while the chip is open in Absolute. No per-frame pass over the objects beyond what the group box does (criterion 36).
5. **Slider behaviour.** Default: one frontend hook for the pointer gestures (click, drag threshold 3 px or 8 px touch, double-click) used by the four sliders, so a fifth gets it for free (criterion 31). The mapping from a position to a value is not duplicated in TypeScript: S3 and S4 keep `rgbOf` and `previewHsv`, S1 and S2 clamp and round with the range constants. The native range inputs may be replaced by `role="slider"` elements if the engines differ on a click (WebKitGTK, WebView2 and WKWebView must give the same result, criterion 17); that is a frontend decision, not an ADR.
6. **Module size.** `ui-core/src/move_entry.rs` and `transform_entry.rs` (548 non-test lines, `0019` flag 7) are at or near the 500-line limit: the reference code goes into a new module (`reference_point.rs`), not into them.
7. **Collisions.** `session/keys.rs`, the commit labels and `MoveEntryChip.tsx` are touched by `0044`, `0020` and `0043`; `useEditorSession.ts` by all of them. Run this slice alone, after #80. Before or after `0020` in either order (customer, 2026-10-10: 0047 does not wait for undo); before `0044` and `0020` gives the smallest rebase (nudge and undo then name the final labels).

## Flagged to the PO

Nothing yet; the architect adds here.
