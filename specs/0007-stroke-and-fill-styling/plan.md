# Plan for Stroke and fill styling

Branch `story/stroke-and-fill-styling`, worktree `/home/marc/workbench/vecmanf-claude/stroke-fill`,
from `main` at `bdf4a11` (`CURRENT_FORMAT_VERSION` 6 after `rectangle-corner-radii`, so this story
takes **7**). Delivered as four stacked PRs (specification, "Dependencies and sequence"; `adrs.md`,
readiness check section 10). PR 1 is invisible to the maker; PR 3 is the first demo.

## Affected crates/modules

| Crate | New or changed |
|---|---|
| `curvyo-document-core` | New `style_model.rs` (`Style`, `Stroke`, `Fill`, `GradientStop`, `StopId`, `Opacity`, `StopPosition`, `DashPattern`, `LineJoin`, `LineCap`, `FillKind`), `style_codec.rs` (keys, absent defaults, reads, writes, open-file validation), `styles.rs` (`edit_style`, `set_fill_mode`, `add_stop`, `remove_stop`, `edit_stops`). `PathSnapshot` and `PrimitiveSnapshot` carry `style: Style` instead of `stroke_width`/`stroke`/`fill`; `PrimitiveSnapshot` drops `Copy`. `path_codec`/`shape_codec` call the style codec. `document.rs`: `CURRENT_FORMAT_VERSION` 7, `document.json` writes one `style` object per object. `split_at_anchor` copies the whole style. Resize commands write the width through the style codec and refuse a width `<= 0` or not finite. |
| `curvyo-render-core` (PR 1) | Reads `style.stroke.width` and `.color` only, so criterion 3 holds. |
| `curvyo-ui-core` (PR 1) | `transform_drag::scale_stroke`, `transform_commit::commit_resize` and `numbers_of` read `style.stroke.width`. |
| `curvyo-render-core` (PR 2, 4) | New `artwork.rs` (one pass over `&[ObjectSnapshot]` in tree order), `dash.rs`, `fill.rs`; later `gradient.rs`. Decorations split out of `build_draw_list`. |
| `curvyo-geometry-core` (PR 2) | One winding-number function for interior hit-testing (closed: real closing cubic; open: straight chord). |
| `curvyo-ui-core` (PR 2, 3, 4) | New `hit_test_object` rule (criterion 27), `select_tool/press.rs`, hover and double-click; `style_panel.rs`, `style_edit.rs` (override preview and dispatch); `StopId` minting; default stops and add-stop position/colour rule. |
| `curvyo-editor-wasm` (PR 2, 3, 4) | `gpu.rs` pure move into `gpu_pipeline.rs`, depth layers (PR 2); `wasm_style.rs`, `session/style.rs`, `session/tolerances.rs` pure move (PR 3); `Vertex` widening, ramp texture, `gpu_paint.rs` (PR 4). |
| `frontend/` (PR 3, 4) | `PropertiesPanel` with the Style section, `useStylePanel.ts`, `App.tsx` re-anchoring, UI wrappers, `react-colorful`; stop editor (PR 4). |
| `docs/` | `design-system.md` rows (PR 3), `technical-debt.md` lines (PR 1, 2, 4). |

## Tasks

### PR 1: model and format version (no visible change)

- [x] 1. `style_model.rs`: the types and the frozen defaults, validated newtypes with
  `try_from` serde; `GradientStop::default_pair` for the 2-stop start (AC 9, 16, 17).
- [x] 2. `style_codec.rs`: keys, absent defaults, lenient reads, `write_style` (Split's copy),
  `write_changes` (skip unchanged values), open-file validation of every present key and every
  stop, with no check of the stop count (AC 3, 5, 6, 9, 13, 16, 25).
- [x] 3. Snapshot field replacement (`style: Style`) and re-pointing in `document-core`,
  `render-core`, `ui-core`, `editor-wasm`, and the test files that read the old fields (AC 3).
- [x] 4. `styles.rs` commands: `edit_style` (one commit for N objects, width 0 is "no stroke",
  colour/opacity/width edit turns a stroke on), `set_fill_mode` (keeps colour and stops; seeds
  stops when a gradient has none) (AC 2, 4, 5, 6, 9, 13, 24).
- [x] 5. Stop commands: `add_stop` (position-ordered insert, refuses a 17th), `remove_stop`
  (refuses below 2), `edit_stops` (batch, one commit, only the named value) (AC 16 to 20).
- [x] 6. `CURRENT_FORMAT_VERSION` 7 with its doc paragraph; `document.json` `style` object;
  golden fixture `styles_v7.curvyo` (all keys, a 3-stop gradient with coincident stops);
  `rotation_v5`/`legacy_corner_radius_v5`/`corner_radii_per_corner` fixtures open with every style
  at its default; version pins follow (AC 3, 25).
- [x] 7. Duplicate, split, join and "Object to path" tests: a copy keeps style and stops and is
  independent; Split copies the style (stops included) to the new object; Join keeps the
  survivor's; conversion keeps the style and no style key is in `ALL_PRIMITIVE_KEYS`
  (AC 30 to 33).
- [x] 8. Resize commands write the width through the style codec; a width `<= 0` or not finite
  is refused before any write (AC 4; readiness check section 3).
- [x] 9. Benchmark re-run with styled objects (`#[ignore]` 200-object frame at rest); note in the
  PR and in `docs/technical-debt.md` if it passes 25 ms (readiness check section 5).

### PR 2: rendering and hit-testing (visible: draw order)

- [ ] 1. `gpu.rs` pure move of pipeline and shader set-up into `gpu_pipeline.rs` (no behaviour
  change; keeps `gpu.rs` under the module limit).
- [ ] 2. `render-core/artwork.rs`: one pass over `&[ObjectSnapshot]` in tree order, each object
  fill then stroke; `Session` hands it the objects with the live node-drag paths substituted;
  decorations split out of `build_draw_list` (AC 26).
- [ ] 3. Stroke on/off, opacity, join, cap (miter limit 4, `Miter` not `MiterClip`) (AC 5, 6,
  10, 11, 12).
- [ ] 4. `dash.rs`: dashes from the document width times the ratios, each dash a real sub-curve
  with its own caps; period under 2 screen px, more than 2000 dashes for one object, or more than
  50 000 in a frame draws solid (AC 7, 8, 9).
- [ ] 5. `fill.rs`: non-zero fill, open paths closed with a chord for the fill only (AC 13, 14,
  15).
- [ ] 6. Depth layers in `gpu.rs`/`gpu_pipeline.rs`: one depth value per paint layer, `Less`
  test, so a translucent stroke covers each pixel once; MSAA sample count of the depth attachment
  equal to the colour target; recorded browser pixel check (AC 6).
- [ ] 7. `geometry-core` winding function; `ui-core` `hit_test_object` rule (criterion 27), press
  order with option B (criteria 28, 29), hover following the press, double-click on a filled
  interior (AC 23, 27, 28, 29).
- [ ] 8. Editor lines over fills: white casing for the selection box, hover box (60% accent),
  preview outline, glyphs, handle lines, guides, pivot; `--hover-box` token (AC 40, 41).
- [ ] 9. `docs/technical-debt.md`: dated line on the `Session` size item; draw-list cache moves
  into this PR if PR 1's benchmark passed 25 ms.

### PR 3: panel with stroke and solid fill (first demo)

- [ ] 1. `session/tolerances.rs` pure move out of `session/mod.rs`.
- [ ] 2. `ui-core/style_panel.rs` (state, mixed values via `BarValue`, scope per tool, subject
  line) and `style_edit.rs` (override preview, dispatch to the object selection) (AC 1, 2, 24,
  36, 37).
- [ ] 3. `wasm_style.rs` binding; commit-on-release, coalesced previews, Escape drops the preview
  (AC 36).
- [ ] 4. Frontend: `PropertiesPanel`, Style section (stroke, solid fill), `ColorAlphaPicker`
  (`react-colorful`), typed-field rules, Paint switch, dash presets (`[6,4]`, `[1,3]`,
  `[6,3,1,3]`; unknown pattern shows "Custom"), disabled and mixed states (AC 4 to 9, 13, 14,
  36, 37).
- [ ] 5. Panel keys and focus: keys never reach the canvas, `Shift+Ctrl+F`, Escape order, focus
  return after a pointer interaction (AC 38).
- [ ] 6. Layout: canvas region and 280 px panel side by side, collapse tab, window minimum
  (measure and raise if needed), document does not move when the panel toggles (AC 39).
- [ ] 7. `design-system.md` rows for the new tokens and components.

### PR 4: gradient

- [ ] 1. `StopId` minting in `ui-core` (the minter serves two id types); default stops via
  `GradientStop::default_pair`; the add-stop rule (widest gap, ramp colour at the position)
  (AC 17, 18).
- [ ] 2. `render-core/gradient.rs`: stable sort by position, the 0- and 1-stop rules, colour at
  t, sRGB-encoded interpolation pinned by a golden ramp (AC 16, 35).
- [ ] 3. `Vertex` widening, ramp texture, shader, `gpu_paint.rs`; `editor-wasm` passes the
  `OrientedBox` values in the fill command (AC 21, 22).
- [ ] 4. Stop editor: gradient bar, thumbs, stop list, Add/Remove, multi-selection by rank, the
  0/1/more-than-16 states (AC 16 to 20, 34, 35).
- [ ] 5. Tester cases from the ADR: rotated 90 degrees, path skewed 30 degrees, "Object to path"
  re-fit of a polygon, coincident stops, zero-size box. One line on the polygon/star box limit in
  `docs/technical-debt.md` (AC 21, 22).

## Validation

- Core logic test-first: unit tests in each new module, integration tests with the
  `tests/` files of the story, golden fixtures `styles_v7.curvyo` (all keys, 3-stop gradient with
  coincident stops) plus the older fixtures reading at their defaults.
- Concurrency: two replicas editing different style properties of the same object, and two
  stops of the same gradient, merge without loss (ADR 0009 section 3).
- Open-file validation: one test per refusal case of `adrs.md` (wrong type, non-finite number,
  `stroke_width <= 0`, opacity or position outside `[0, 1]`, unknown join/cap/kind string, odd or
  negative or all-zero dash list, a stop missing a field) and the acceptance of 0, 1 and 17 stops.
- The full CI list from `.github/workflows/ci.yml` on the exact head SHA before each report.
- PR 2: draw-list and native tests for everything pure; a recorded browser pixel-read check for
  the single-coverage stroke and the gradient; measure the depth attachment's memory on the
  customer's machine.
- PR 3 and 4: Browser pane check of the panel at 800 x 600 and wider; contrast figures from
  `design-system.md` re-measured.
