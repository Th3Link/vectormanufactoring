# ADRs for "Style panel rework"

This slice changes the panel of `0007-stroke-and-fill-styling`, deletes the
gradient fill, and allows odd-length dash lists in the format. It adds **no new
crate, no new runtime dependency, no trait and no generic**. It **removes** one
frontend dependency (`react-colorful`) and two popup wrappers. Reference state:
`main` at `744dfad` (0007 PR 1 to 4 merged). Every PR starts after
`advanced-selection` (PR #61) merges, because #61 changes
`ui-core/hit_test_object.rs`, `editor-wasm/session/*` and splits `wasm_api.rs`.

Where this file and `specs/0007-stroke-and-fill-styling/adrs.md` disagree, this
file wins. The 0007 notes on the gradient stop model, `StopId`, the ramp, the
gradient frame and the mergeable stop list are void.

## Depends on

- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  fully resolved style per node. A fill is now None or Solid. **§10 lists
  "linear/radial gradients" in the SVG subset we model; that item now goes to
  the passthrough bag on import. Needs a dated amendment line in ADR 0002 §10
  (lead: add it in PR 1; the architect was not cleared to edit ADR files in this
  pass).**
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  never partially read; a bump wherever an older reader would misread or
  wrongly refuse a file. Decides decision 1 (no bump) and decision 2 (bump).
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md): drag
  previews and picking mode are ephemeral; one register per control (0007).
- [ADR 0001 §3 to §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  rules in `ui-core`, the host draws; per-interaction data as small messages.
- [ADR 0003 §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  dashes stay display tessellation in `render-core`.
- [`0007` adrs.md](../0007-stroke-and-fill-styling/adrs.md): register split,
  "a write that does not change the value writes nothing", `StyleEdit::apply_to`
  as the one rule for preview and commit, the depth layers, the hit order. Kept.

## Feature-local decisions

- **2026-10-08: (1) gradient removal and old files (criteria 49 to 54).**
  - **What is deleted.** `document-core`: `gradient_ramp.rs` (`ramp_at`,
    `sorted_stops`); in `style_model.rs` `FillKind`, `GradientStop`, `StopId`,
    `StopPosition`, `StyleParamError::StopPositionOutOfRange`, `Fill.kind`,
    `Fill.stops` (`Fill::paints` becomes `enabled`); in `styles.rs` `FillMode`,
    `FillModeTarget`, `StopChange`, `StopEdit`, `MIN/MAX_GRADIENT_STOPS`,
    `set_fill_mode`, `add_stop`, `remove_stop`, `edit_stops`; in `style_codec.rs`
    the stop list reads and writes; in `style_validation.rs` the stop checks.
    Fill on/off becomes `StyleEdit::FillEnabled(bool)`, beside `StrokeEnabled`.
    `render-core`: `gradient.rs`, `DrawList::gradients`/`push_gradient`/
    `gradient_attributes`, the `frames` argument of `build_artwork` (now
    `build_artwork(objects, view)`), the gradient exports in `lib.rs`.
    `editor-wasm`: `gpu_paint.rs`; the ramp texture, ramp bind group, sampler
    and the `gradient` vertex attribute in `gpu_pipeline.rs`/`gpu.rs` (the vertex
    shrinks back by four floats); `session/draw.rs::gradient_frames`;
    `session/stops.rs`; the selected-stop state in `session/mod.rs`; the stop
    fields of `style_view.rs`; the stop calls of `wasm_properties_panel.rs`
    (`set_fill_mode` becomes `set_fill_paint(on)`). `ui-core`: `style_stops.rs`;
    `StopField`, `parse_position_percent`, `fill_mode_from_name` in
    `style_entry.rs`; `StyleEditor::preview_stops`; `StopsPanel` in
    `style_panel.rs`; `AnchorIdMinter::mint_stop` (the "minter is misnamed" debt
    item goes with it). Frontend: `GradientEditor`, `GradientBar`, `RampTrack`,
    `StopRows`, `StopThumb`, `useThumbDrag`, the gradient parts of
    `styleOptions`, `StyleIcons`, `Swatch`, `useStylePanel`, `FillSection`,
    `index.css`. Tests: `editor-wasm/tests/gradient_frames.rs`,
    `ui-core/tests/style_stops.rs`, every gradient case of the three
    `acceptance_0007_pr4_tester.rs` files (delete a file when nothing else is
    left in it), of `style_format.rs`, `stroke_fill_styling.rs`,
    `acceptance_0007_pr1_tester.rs`, `style_artwork.rs`, `style_panel*.rs`,
    `hit_test_object.rs`. `docs/technical-debt.md`: the gradient paragraphs of
    "Rotation is a stored angle", "Canvas performance" (PR 4 sentence) and the
    0007 notes (PR 1 review `fill_stops`, PR 2 "not painted until PR 4", all PR 4
    gradient items; reword "`DrawList` lives in `glyphs.rs`" without gradients).
    `docs/design-system.md` is the ux-engineer's (criterion 58), not PR 1's.
  - **Old files: tolerate and ignore on read, drop on the next fill write.**
    Rejected: *refuse* (validation already refuses unknown `fill_kind` strings,
    so it costs nothing) because the customer's own test files from the 0007 PR 4
    build would stop opening with a "damaged" message; *convert on open* because
    it writes on open (criterion 53: the file is not changed by opening).
    The rule, in one new private module `document-core/src/legacy_fill.rs`
    (the only place the words `gradient`, `linear`, `radial`, `fill_kind` and
    `fill_stops` remain): read: a fill paints only if `fill_enabled` and
    `fill_kind` is absent or `"solid"`; any other `fill_kind` reads as
    `enabled = false`. Write: `style_codec::write_changes` calls
    `legacy_fill::forget(meta)` whenever it writes any fill key, which deletes
    `fill_kind` and `fill_stops` if present, in the same commit. Validation:
    `fill_kind` stays one of `solid`/`linear`/`radial` (a version-7 file that was
    damaged stays damaged); `fill_stops` is accepted in any form and never read.
    Copy keeps the keys (structural copy), Split's new half does not get them
    (it is written from `Style`), Join and "Object to path" keep the meta map.
    All of these keep showing no fill, which is consistent.
  - **No `format_version` bump for the removal.** A version-7 reader reading
    what this build writes sees only keys it knows; this build reading any
    version-7 file reads every key it still knows and ignores the two legacy
    keys by rule. Nothing is misread in either direction.
  - **Fixtures.** Keep exactly one: `styles_v7_mergeable_stops.curvyo`, renamed
    to `legacy_gradient_v7.curvyo` with its bytes unchanged (`git mv`): it holds
    every v7 key and a 3-stop gradient in the mergeable form, which is the form
    the customer's PR 4 build wrote. Delete `styles_v7.curvyo` (same document,
    regular stop list, only ever written by tests). Tests: opens without error;
    stroke and solid values read as stored; the gradient object reads fill off;
    the exported bytes after open equal the bytes before (no op); Paint Solid
    on it gives Solid in the stored `fill` colour (black if absent) and removes
    both keys in one commit; a save without a fill edit keeps both keys.
    `rotation_v5`, `legacy_corner_radius_v5`, `corner_radii_per_corner`,
    `primitives_v3`, `paths_v2` stay.

- **2026-10-08: (2) dash list: odd lengths are stored as typed; this needs a
  bump (criteria 29 to 33).** `DashPattern::new` accepts 1 or more finite
  entries `>= 0` with a sum `> 0`, any length, odd or even (empty = solid).
  1 to 16 entries and 0 to 1000 per entry are **edit-time** limits in `ui-core`;
  a file may hold more (criterion 33). The renderer expands an odd list to
  twice its length (SVG). Rejected: doubling an odd list on write (no bump, but
  the line would show `1 2 4 1 2 4` after typing `1 2 4`, against criterion 30).
  A version-7 reader refuses an odd list as `Damaged`, the wrong message, so the
  PR that first writes an odd list takes `CURRENT_FORMAT_VERSION + 1`
  (**next free at merge, after 0016's 8**, by the standing merge rule). Migration from 7 is
  empty: every stored v7 list is even and stays valid. Fixture `dash_vNEXT.curvyo` (named after the version it takes at merge):
  an odd list, a 17-entry list and a zero "on" entry. The parser
  (`ui-core::style_entry::parse_dash_text`) splits on **runs of spaces only**;
  the decimal mark is the point. A comma is an error, neither separator nor
  decimal mark (`1,2,4,2`, `1, 2` and `1,5` are refused, criterion 30), so no
  text is ambiguous. Display: entries separated by
  one space, Rust's shortest float form (`0.5`, `6`). A zero "on" entry draws a
  zero-length dash, so round and square caps show a dot (criterion 32; today
  `dash.rs` skips it, see the technical-debt item "SVG import and dashes"). If
  `lyon` draws no cap on a zero-length sub-path, `dash.rs` adds the cap shape
  itself in the same layer. A test pins it.

- **2026-10-08: (3) eyedropper (criteria 22 to 27).** **Source: the stored
  colour of the topmost painted object** (the spec's default A). Rejected:
  *rendered pixel* (a GPU read-back in `editor-wasm`, wasm32-only and untestable
  natively, mixes antialiasing and overlaps, async in `wgpu`); *the browser
  `EyeDropper` API* (Chromium only: WebKitGTK on Linux and WKWebView on macOS do
  not have it, it samples the screen, which is out of scope, and it opens its
  own system UI). No new dependency.
  - `ui-core`, new `colour_pick.rs`: `pick_colour(objects, point, tolerance) ->
    Option<(Color, Opacity)>`. Topmost first: the first object whose stroke
    paints (`enabled`, any opacity) and whose outline is within `tolerance`
    gives its stroke RGBA; else, if its fill paints and contains the point, its
    fill RGBA; else the next object down. It reuses `hit_test_object.rs`'s
    `distance_to_object`, `fills_point` and cheap reject (made `pub(crate)`),
    not a copy. Unlike the Select rule this is topmost-first, not
    nearest-outline. Tolerance: the Select tool's object tolerance
    (`Session::object_tolerance()`, 8 px, from #61). Overlays are never in the
    object list, so they cannot be picked.
  - `editor-wasm`: `Session` holds `colour_pick: Option<PaintTarget>` (stroke
    or fill), ephemeral. While set, a primary-button canvas press goes to the
    pick and nothing else (no tool sees it). Pan and zoom (wheel, middle-drag,
    Space-drag, keys) are navigation, not tool input: they keep working and do
    not end picking. A right press ends picking and writes nothing; the host
    suppresses the context menu (criterion 26). A hit writes `StyleEdit::StrokeRgba`/`FillRgba` to
    the style scope in one commit and clears the mode; a miss keeps it. A
    pointer move stores the hovered result for the view. `set_tool` clears it.
    Facade: `begin_colour_pick(target)`, `end_colour_pick()`, and the view
    fields `pick_target` and `pick_hover_hex`. The host ends picking on Escape
    and on a press in the panel (criteria 26, 60) and draws cursor and hover
    chip. Native session tests cover criteria 24 to 26 and 59.

- **2026-10-08: (4) value field (criteria 34 to 48).**
  - **Scales are pure Rust** in a new `ui-core/value_scale.rs`: an enum
    `ValueScale { StrokeWidth, Opacity }` (stroke-markers adds `MarkerCount`)
    with `value_at(p)`, `position_of(v)`, `round(v, Grid)` and
    `step(v, steps, Grid)`; `Grid` is `Normal | Coarse | Fine`. No trait, the
    set is closed. Native tests use the check values of criterion 46 and the
    grids of 47. Reason: the frontend has no test runner, and the curve and
    rounding are what the tester can pin exactly.
  - **The host owns the gesture only**: 3 px threshold, `setPointerCapture`,
    `p = p_base + factor * (x - x_base) / W` with re-basing when a modifier
    changes, the absolute `p` for a mixed field, keys. It sends `p` (or a step
    count) and the grid; Rust maps, rounds and previews:
    `preview_value_field(field, p, grid)`, `step_value_field(field, steps, grid)`,
    then the existing `commit_style_preview` / `cancel_style_preview`. The view
    carries each field's bar position and its shown text (Rust formats: width up
    to 3 decimals without trailing zeros, opacity as an integer).
  - **Pointer capture, not pointer lock.** Capture keeps events flowing outside
    the field and the window while the button is down, which criterion 37 needs.
    Rejected: pointer lock, which hides the cursor, shows a browser banner in
    the web target, needs a user gesture and is ended by Escape (taken by
    criterion 40).
  - **Row visibility comes from the committed document; values come from the
    preview.** Otherwise a width drag to 0 would remove the row mid-drag
    (criterion 8). `Session::style_panel_state` computes the visible flags from
    the objects before `StyleEditor::apply_to`. Criterion 8 needs no new
    register: `StyleEdit::StrokeWidth(0)` already writes only
    `stroke_enabled = false` and keeps `stroke_width > 0` (0007, register
    granularity 3), so Solid restores the last width; a stored width is never
    `<= 0` (validation), so the 0.25 mm fallback is the absent-key default.
  - **`NumberField` is used by the panel only** (Stroke width, opacity, hex,
    stops), not by the bars. It becomes `EntryField` (rename; the `disabled`
    prop goes, criterion 3): the typed-text control with the validation
    message and the Enter/Tab/Escape rules. The hex field, the dash line and a
    value field in typing mode all render it. `ValueField.tsx` and
    `useValueDrag.ts` are new. One text-entry implementation, not two.

- **2026-10-08: (5) colour: one source of truth, no model change (criteria 11
  to 16).** Stored: `Color` (RGB `u8`) plus `Opacity` (`f64` in `[0, 1]`) per
  paint, as in 0007. Hex and percent are views of it, formatted in Rust
  (`ui-core::style_entry`): `AA = round(255 * a)`, percent `= round(100 * a)`,
  both half up (`f64::round` on non-negative values). `render-core`'s
  `paint()` already uses the same `AA`. Writes store what was typed exactly:
  `AA / 255` from the hex field, `N / 100` from the Opacity field. Nothing is
  normalised on read, and the "unchanged value writes nothing" rule keeps an
  off-grid file value as it is. An edited 8-digit hex rewrites the alpha even
  when `AA` is unchanged (0.5 becomes 128/255): accepted, it is what the field
  shows. New `StyleEdit::StrokeRgba(Color, Opacity)` and `FillRgba`, used by
  4/8-digit hex and the eyedropper, so RGB and alpha land in one commit.
  **HSV conversion** for the inline picker is a new `ui-core/colour_hsv.rs`
  (`hsv_to_rgb`, `rgb_to_hsv -> (Option<hue>, s, v)`; `None` for grey and
  black, so the host keeps its last hue, criterion 20). The host sends h, s, v;
  Rust rounds to `u8` and previews `StyleEdit::StrokeColor`/`FillColor`.

- **2026-10-08: (6) panel content and "no popups" (criteria 1 to 10, 55, 56).**
  - The empty panel, the hidden rows and the Markers visibility of
    `stroke-markers` are flags of the `ui-core::style_panel` view (a pure
    function of scope and snapshots, tested natively). The host renders flags
    and has no `disabled` prop anywhere in the panel.
  - **`react-colorful` is removed.** Its sliders cannot be renamed (criterion
    17 needs "Saturation and value"), it has no gesture start/end (criteria 18,
    19 need preview on key-down and commit on key-up), and it always draws a
    thumb (criterion 21 needs none for mixed). PR 3 of 0007 already patched the
    names after mount (technical-debt item, removed with it). The inline picker
    is about 150 lines of our own TSX over decision 5's conversion.
  - Deleted: `ui/popover.tsx`, `ui/select.tsx`, `ColourPopover.tsx`,
    `ColorAlphaPicker.tsx` (replaced by an inline colour block), `DashSelect.tsx`
    (replaced by the existing `ToggleGroup` plus the dash line). Kept:
    `ui/tooltip.tsx` (text only), `ui/toggle-group.tsx` (Radix `RadioGroup`, no
    layer), `ui/alert-dialog.tsx` (the error dialog, not in the panel).
  - **Enforcement:** the panel's components move to `frontend/src/components/
    panel/`, and `.oxlintrc.json` gets an override for that folder with
    `no-restricted-imports`: the paths `@/components/ui/alert-dialog` (and any
    later popover/select/menu wrapper) and, from `radix-ui`, the names
    `Popover`, `Select`, `DropdownMenu`, `ContextMenu`, `Dialog`, `AlertDialog`,
    `HoverCard`, `Menubar`, `NavigationMenu`. CI already runs `npm run lint`. If
    the pinned oxlint lacks `importNames`, a path-only rule plus the tester's
    DOM check of criterion 55 is enough.

- **2026-10-08: (7) PR split, in order, each a `story/` PR, all after #61.**
  1. **Gradient removal** (decision 1). All five crates plus frontend; no bump;
     the panel keeps its current look with Fill None/Solid. Criteria 49 to 54.
  2. **Panel content, colour and dash** (decisions 2, 5, 6): empty and hidden
     rules, 8-digit hex, inline picker, preset group plus dash line, the format
     bump (next free at merge, after 0016's 8), `react-colorful` and the popup wrappers deleted, the lint
     rule, Escape order. Criteria 1 to 21, 28 to 33, 55 to 58, 60.
  3. **Value fields** (decision 4). Criteria 34 to 48, 59, 61 (reset slot: the
     defaults live beside the scales in `ui-core::value_scale`; a reset is one
     `StyleEdit` through `edit_style`, which already writes nothing for objects
     at the default).
  4. **Eyedropper** (decision 3). Criteria 22 to 27.
  PR 3 and 4 touch the same crates (`ui-core`, `editor-wasm`, frontend panel),
  so they are not parallel. `stroke-markers` PR 1 (`document-core`,
  `render-core`) may run in parallel with PR 3 or 4 once PR 2 has merged.

## Flagged to the lead

1. **ADR 0002 §10 amendment** (gradients leave the modelled SVG subset, go to
   passthrough), one dated line, in PR 1.
2. **PO edits, small:** criterion 30 (done 2026-10-08: spaces only); criterion
   53 can name the trigger exactly: any write of a fill key (Paint, colour,
   opacity, eyedropper) drops the gradient data in the same commit.
3. **PO, one open rule, default taken:** criterion 9 says a stroke colour,
   opacity or width edit turns an off stroke on. For the fill, `0007` never did
   (a fill colour edit leaves the fill off). Default: unchanged, the fill is
   only turned on by its Paint switch. Criterion 15's "same rules" should say so.
4. **`docs/technical-debt.md`** gains, in PR 1, one item: "version-7 files may
   hold gradient keys; `legacy_fill.rs` hides and drops them; delete it once no
   such file is expected (customer's call)". PR 2 resolves "SVG import and
   dashes" (zero "on" entries).
5. **No customer question** from the architecture: the customer's choices
   (hard delete, eyedropper, no popups, dash line, log value fields) are all
   buildable as stated; the spec's open questions stay with the PO.
