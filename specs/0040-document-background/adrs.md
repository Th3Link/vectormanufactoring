# ADRs for "Document background"

The background is two new registers in the document root, one format bump, a
changed bottom layer in the frame's draw list with a checkerboard drawn by the
fragment shader, one more source in the eyedropper rule, and a Background
block that reuses 0017's colour components. **No new crate, no new
dependency, no new ADR, no trait, no generic.** Reference state: `main` at
`4dd8352` (0017, 0018 and 0030 merged as #78).

## Depends on

- [ADR 0002 §1, §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  the model is our own types, and the Loro backing stays inside
  `document-core`. The background is a property of the document root and is
  not a node, so §5 (tree, z-order, per-node style) does not apply.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  each background edit is one commit (criterion 22).
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  a version-9 reader would show the default grey for a red document. That is
  a silent partial read of visible content, so the version goes up
  (decision 3). This differs from `display_unit` (0015 decision 1), which is
  presentation only.
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md):
  last-writer-wins registers. The preview during a drag is ephemeral.
- [ADR 0001 §4, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the background is part of the WebGL draw list. Per-frame data goes to the
  GPU as a uniform, not as geometry.
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md): the
  existing crate edges are enough (`render-core -> document-core`,
  `ui-core -> document-core`).
- [`0015` adrs.md](../0015-document-size-and-rulers/adrs.md) decisions 11 and
  13 and the PR 2 note: the document area is the bottom layer of
  `Session::frame_draw_list`, and `draw_list` stays artwork plus overlay.
  The root-register pattern comes from `display_unit.rs`.
- [`0017` adrs.md](../0017-style-panel-rework/adrs.md) decisions 3, 4 and 5:
  the eyedropper takes the stored colour, the value field and its scales are
  in `ui-core::value_scale`, hex and percent are views of `Color` plus
  `Opacity`, and writes store `AA/255` or `N/100` as typed.
- [`0030` adrs.md](../0030-document-size-presets/adrs.md): `session/document.rs`,
  `DocumentSection.tsx`, `useDocumentPanel.ts`.

## Feature-local decisions (2026-10-10)

1. **Model: a new module `curvyo-document-core/src/document_background.rs`.**
   Its one job is the document's background value and its two root registers
   (read, validate, write), which mirrors `display_unit.rs`.
   `document_size.rs` (730 lines, size commands) is the wrong place and is
   already too long. Public: `enum BackgroundPaint { None, Solid }`,
   `struct DocumentBackground { paint, color: Color, opacity: Opacity }`
   (the colour types of an object's fill, criterion 1),
   `DocumentBackground::DEFAULT` (Solid, `#E8E8EB`, `Opacity::OPAQUE`),
   `Document::background() -> DocumentBackground` and
   `Document::set_background(DocumentBackground) -> bool`. There is no
   `BackgroundEdit` enum and no new `StyleEdit` variant: the session builds
   the new value with struct update (`DocumentBackground { paint: Solid,
   ..current }`), so a third edit vocabulary would add nothing.
   `render-core`'s `theme::CANVAS_BG` stays as a const built from
   `DocumentBackground::DEFAULT`, so `#E8E8EB` is written in one place.
   `#79` tests use the constant.

2. **Registers and encoding (criteria 3, 6, 45).**

   ```text
   root map, new keys (each its own LWW register):
     background_paint : "none" | "solid"                      absent = "solid"
     background_color : [r, g, b, a]  r, g, b I64 0..=255,    absent = [232, 232, 235, 1.0]
                                      a F64 finite in [0, 1]
   ```

   The colour is **one** register holding one Loro list value. A list value
   (not a container) is replaced as a whole, so concurrent writes never mix
   channels (criterion 45). Red, green and blue are integers and the alpha is
   the same `Opacity` fraction an object's fill stores. So the hex and percent
   rules of 0017 decision 5 apply unchanged: `AA/255` from the hex field and
   the eyedropper, `N/100` from the Opacity field, `AA = round(255 a)`, and
   nothing is normalised on read (criteria 6, 19).
   Options that lost:
   - *Packed RGBA bytes (one `u32`):* typing 50 % would store 128/255, not
     0.5, against criterion 19. It would also be a second alpha rule next to
     the objects' rule.
   - *Four fractions in [0, 1] (the spec's draft wording):* this gives a
     second encoding of `Color`, and RGB would need rounding on every read.
   - *RGB and alpha as two registers (as on objects):* criterion 45 forbids
     channels from two peers mixing.

   The cost is accepted: concurrent Opacity and picker edits keep only one of
   the two edits (last writer wins).
   `Document::new` writes neither key, because absent means the default.
   `document.json` gains `background: { paint, color: [r, g, b, a] }`.

3. **Format: `CURRENT_FORMAT_VERSION + 1`, next free at merge.** On `main` it
   is 9 today. Neither #79 nor #80 bumps it, so the number is **10** unless
   0023 `groups` merges first. Whichever merges first takes 10, and the other
   renumbers its constant, fixtures and notes (the standing rule in
   `document.rs`). The migration from 9 is empty: absent keys read as the
   default, and opening writes nothing (criterion 4). The `document.rs`
   comment on the constant gets the usual paragraph.

4. **Validation on open (criterion 7): strict, as for the marker keys.**
   `Document::from_loro_snapshot` calls
   `document_background::validate(&loro)` after `validate_path_tree`. A
   present key that is malformed gives `OpenError::Damaged`: a paint that is
   not one of the two strings, or a colour that is not a 4-entry list of
   three integers from 0 to 255 and one finite double in [0, 1]. An absent
   key is valid. The **read** stays lenient: a malformed value in a merged,
   never-validated replica reads as that register's default and writes
   nothing (0018 decision 1). This is stricter than the size registers, which
   fall back to A4, as the customer asked.

5. **Command (criteria 17, 22, 42, 43): `Document::set_background(value) ->
   bool`.** It compares the value with the stored one and writes only the
   register that differs (exact `Color` equality and exact `f64` equality of
   the alpha). It commits once with the label `set_document_background`, or
   returns `false` and writes nothing. Because it writes per register, a
   paint edit and a colour edit from two peers both survive (criterion 45).
   `resize`, `fit_to_content`, presets, `set_display_unit` and the clipboard
   never touch the keys. A test asserts that for 42 to 44.

6. **Drawing (criteria 9 to 13, 47): the fragment shader draws the
   checkerboard, on one quad.** `build_document_area(size, background, view,
   dpr)` returns:
   - Opaque Solid: one quad in that colour. This is the same list as today,
     so every existing test passes unchanged (criterion 9).
   - None: one quad marked as checkerboard.
   - Translucent Solid: the checkerboard quad, then a colour quad as a second
     layer, blended by the existing pipeline.

   `DrawList` gains a **checkerboard prefix** (`checker_end: usize`): the
   first vertices are painted by a second fragment entry point, `fs_checker`,
   in a third pipeline with the same vertex layout and the same depth rule
   as the artwork pipeline. `extend` keeps the prefix of `self`, and the
   prefix of `other` must be empty. That holds because only the document area
   has one, and it always comes first. The `ScreenTransform` uniform gains the
   snapped document corner in device px and the cell size in device px. The
   cell is `round(8 × dpr)` device px, at least 1, which is fixed on screen and
   anchored at the corner. The two tones `CHECKER_A` and `CHECKER_B` are
   constants in `theme.rs`. The tone at a pixel is the parity of
   `floor((p − corner) / cell)`, with even meaning A. A pure
   `checker_tone(device_px, corner_px, cell_px)` in `document_area.rs` holds
   the same formula for native tests, and the WGSL mirrors it in one line.
   The cost: no geometry per cell, one extra draw call only while a checker
   shows, and one integer test per fragment.
   Options that lost:
   - *CPU triangles per cell:* about 16 000 quads per frame at 1080p, rebuilt
     on every frame and every zoom, which is against criterion 47.
   - *A per-vertex paint flag:* 17 `Vertex` literals change, and every vertex
     grows by 4 bytes for one quad.
   - *A CSS layer under a transparent canvas:* two compositors that must stay
     in sync on every pan (the reason 0015 drew the rulers with rAF), and
     native tests could not see it.

   The default is still drawn only in `frame_draw_list`. `Session::draw_list`
   is unchanged, and a later export or thumbnail path must use
   `frame_draw_list` (0015 note).

7. **Pen knockout (criterion 46).** `background_at(size, background, point)`
   returns the opaque background colour, or the background composited over
   `CHECKER_A` for None (`CHECKER_A` itself) or a translucent colour. It
   returns `PASTEBOARD_BG` outside. Containment is `DocumentSize::contains(point)`,
   new in `units.rs`: closed, with tolerance zero, as criterion 34 states
   explicitly. The eyedropper uses the same function, so the knockout and the
   pick agree on the edge. The colour conversion is `artwork.rs`'s private
   `paint()`, made `pub(crate)` and reused, not copied.

8. **Eyedropper (criteria 28 to 38, 48): one function, one more source.**
   `ui-core::colour_pick::pick_colour(objects, point, tolerance, size,
   background)` first runs the object walk exactly as today. If it finds
   nothing, it returns the stored background colour when the paint is Solid
   and `size.contains(point)`. Otherwise it returns `None`, which covers the
   pasteboard and a None background. `PaintTarget` gains `Background`, which
   serves both as the source of a pick (chip "Background #RRGGBBAA") and as
   the target of the Background block's button. The host name is
   `"background"`. `session/colour_pick.rs` maps that target to
   `set_background(DocumentBackground { paint: Solid, color, opacity })`.
   A pick equal to the stored value writes nothing and still ends picking.
   Hover and press pass the **committed** background, because `begin_colour_pick`
   already flushes previews. A test pins that the background branch adds one
   containment test after the walk and nothing else. 0023 changes the object
   walk (into groups), and the background branch stays after it.

9. **Panel (criteria 14 to 27): reuse the pure functions, add no shared
   "colour-edit core".** A new `editor-wasm/src/session/background.rs` holds
   `set_background_paint`, `set_background_hex` (through `parse_hex`, which
   keeps the alpha for 3 and 6 digits), `preview_background_hsv`
   (`hsv_to_rgb`, which keeps the alpha), `preview/step_background_opacity`
   (`ValueScale::Opacity`, with the reset slot at its default), and
   `commit/cancel_background_preview` and `background_view`. It reuses
   `hex_text`, `ValueScale::Opacity::shown` and `rgb_to_hsv`. The preview
   is `Option<DocumentBackground>` in one session field (ephemeral,
   ADR 0009 §2). `frame_draw_list` and the view read the preview when there
   is one. Row visibility comes from the committed paint (0017 decision 4).
   The preview survives a selection change and commits on release (criterion
   23). `Session::new` and `open` start without one (criterion 8).
   `wasm_document.rs` gets the bindings and stays under 250 lines.
   Frontend: `ColourBlock.tsx`, `ColourPicker.tsx` and `ValueField` take
   callbacks instead of `StylePanelApi`. A new `BackgroundBlock.tsx` and
   `useBackgroundPanel.ts` render them. Nothing is added to
   `useEditorSession.ts` (1 662 lines).

10. **The background is not an object (criteria 40 to 44).** It is not in the
    objects tree. Selection, Select all, marquee, lasso, bounds, Fit,
    booleans, path tools, transforms, Delete, Duplicate, Move and the
    clipboard all go through `object_ids`/`objects()`, so they cannot see it.
    One session test per criterion pins it. Hit-testing in `ui-core`
    (`hit_test*.rs`) is unchanged.

11. **Fixtures (golden, `curvyo-document-core/tests/fixtures/`), named after
    the version taken at merge:**
    - `background_v10.curvyo`: paint None, colour `[47, 111, 238, 128/255]`,
      for criterion 6.
    - Four damaged files, one per rule of criterion 7:
      `background_paint_unknown_v10`, `background_color_three_v10`,
      `background_color_range_v10` (alpha 1.5) and
      `background_color_type_v10` (red as a double).
    - `format_version_1`, `dash_v9`, `markers_v9` and `compound_v8` open with
      the default, and their bytes on disk are unchanged after Open
      (criterion 4). That test goes in `curvyo-storage-io`.

12. **Delivery: one branch `story/document-background`, one PR, these
    milestones.** Each milestone leaves the gate green.
    1. `document-core`: decisions 1 to 5 and 11. The architect reviews the
       branch diff, because it changes the file format.
    2. `render-core` and the GPU: decisions 6 and 7.
    3. `ui-core` and `editor-wasm` session: decisions 8 to 10.
    4. Frontend: the block, the chip and the announcement. Then the UX review
       and the tester pass.

## Collisions

- **#79 (path tools):** adds `render-core/src/pen_cue.rs`, which calls
  `background_at(document_size, point)`, and changes `pen_preview.rs`,
  `theme.rs`, `lib.rs`, `session/mod.rs`, `session/draw.rs`, `wasm_api.rs`,
  `ui-core/hit_test.rs` and `docs/design-system.md`. Decision 7 changes that
  signature, so whichever merges second updates the call sites. It has no
  format bump.
- **#80 (multi-object transform / group box):** changes `theme.rs`,
  `render-core/lib.rs`, `session/mod.rs`, `session/draw.rs`,
  `useEditorSession.ts`, `Canvas.tsx` and `docs/design-system.md`. These are
  rebase-level conflicts. It has no format bump.
- **0023 groups:** the format number (decision 3), the `from_loro_snapshot`
  validation chain and the `document.json` view, `colour_pick.rs` (the walk
  into groups, decision 8) and `panel_content`. They are not built in
  parallel, because both touch every core crate except `geometry-core`.
- **Recommendation:** start after #79 and #80 merge, so the branch begins on
  the final `background_at` callers.

## Flagged to the PO (defaults taken)

1. **Criterion 7 wording:** "the colour is not a list of exactly four entries:
   three whole numbers from 0 to 255 and one finite number from 0 to 1"
   (decision 2). The draft says "four components 0 to 1".
2. **Criterion 47 test:** count the artwork layers plus the checkerboard
   prefix. With None the list has the same length as with an opaque colour,
   plus the prefix. A translucent colour adds one layer.
3. **Criterion 12:** the cell is `round(8 × dpr)` device px. At a ratio of
   1.1 the cell is 9 device px (8.2 CSS px). The tests run at ratios 1 and 2.
