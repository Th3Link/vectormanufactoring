# ADRs for "Document size and rulers"

Rewritten 2026-10-09. The 2026-10-05 version described pages, which
[ADR 0012](../../docs/adr/0012-pages-in-the-document-model.md) proposed and
the customer rejected. Nothing in this feature depends on ADR 0012.

**This feature adds no crate, no dependency, no ADR and no `format_version`
bump.** The document keeps its single root map and its single object tree.
The size registers exist since `0001-project-file-foundation`. Part C adds one
root register that older builds read correctly by ignoring it (decision 1).

## Depends on

- [ADR 0002 §2, §3, §4](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  mm is the stored unit, units are newtypes, Y grows down. The display unit
  (Part C) is presentation only and never changes a stored length.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  resize, fit and unit change are each one commit (AC 19, 27, 36).
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  old files open without migration. A newer file is refused only when the
  version is raised, and this feature does not raise it (decision 1).
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md):
  width, height and display unit are separate last-writer-wins registers.
  The view, the rulers and the pointer marker are ephemeral and never written.
- [ADR 0001 §4, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the document area and the pasteboard are drawn in the WebGL draw list. The
  rulers are docked chrome next to the canvas, not a drawing of the document,
  so §4 does not apply to them (decision 8). Ruler data crosses the wasm
  boundary as scalars and a short label list, and only when it changes (§5).
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md): the
  existing crate edges are enough. `render-core` still sees only
  `ViewTransform` and document types, never `ui-core`.
- [`specs/0001-project-file-foundation/adrs.md`](../0001-project-file-foundation/adrs.md):
  the minimal root record (`size_width_mm`, `size_height_mm`) and the A4
  default stay as they are.
- [`specs/0004-canvas-navigation-and-selection/adrs.md`](../0004-canvas-navigation-and-selection/adrs.md):
  `Viewport` in `ui-core` is the only view state, and all screen to document
  conversion happens in Rust (AC 1, 2, 9).

## Feature-local decisions (2026-10-09)

1. **Storage. No format bump.** Width and height stay in the root registers
   `size_width_mm` and `size_height_mm`. Part C adds one root register,
   `display_unit`, a string `"mm"`, `"cm"` or `"in"`. An absent or unknown
   value reads as mm. `document.json` gains `display_unit` next to `size`.
   No bump is needed: a version-7 build ignores the key and shows mm, which is
   a correct view of the same document. Loro keeps the key when that build
   saves, so nothing is lost. This differs from the earlier bumps, where an
   older build would have drawn geometry wrongly. AC 38's "format_version is
   raised" changes accordingly (flag 1). The decision also removes any
   version-number race with `0016-boolean-operations`.
2. **Old and damaged size values open, and are not rewritten (AC 13).**
   `Document::size()` returns the A4 default for both axes if either value is
   missing, not a finite number, or outside `MIN_DOCUMENT_MM ..=
   MAX_DOCUMENT_MM`. Today the code checks each axis separately and accepts
   any double, so this changes. Opening writes nothing. Goldens:
   `format_version_1.curvyo` and the v7 goldens open at 210 × 297 mm, a new
   v7 fixture with damaged size values opens at A4, and a new fixture with
   `display_unit = "in"` round-trips.
3. **Units are types (AC 33–35).** `document-core` gets
   `enum DisplayUnit { Mm, Cm, In }` with `mm_per_unit()` (1, 10, exactly
   25.4) in its own module `display_unit.rs`, because `units.rs` is near
   500 lines. `Length::from_unit(f64, DisplayUnit)` and
   `Length::in_unit(DisplayUnit) -> f64` are the only conversions. A value
   in the display unit is a bare `f64` only between text and `Length`, inside
   one parse or format function. Parsing and formatting live in `ui-core`
   (`display_unit_text.rs`). Parsing reuses `parse_entry_number`, which
   already accepts a decimal comma, spaces and U+2212. Formatting uses 3
   decimals for mm and 4 for cm and in, without trailing zeros, and never
   writes to the document.
4. **Resize keeps the centre (AC 17–19).** `Document::resize(DocumentSize)
   -> Result<bool, DocumentSizeError>` lives in a new `document_size.rs`
   (`document.rs` is near 400 lines). It refuses a non-finite size or one
   outside the limits with a typed error. A size equal to the current size
   within 1e-9 mm returns `Ok(false)` and writes nothing. Otherwise it writes
   both registers and moves every object by `((w1 − w0)/2, (h1 − h0)/2)` in
   one commit labelled `resize_document`. The per-kind move inside
   `translate_objects` (`translate_path_meta`, `translate_primitive_meta`)
   becomes one private helper used by translate, duplicate, resize and fit.
   It is not copied. Handles are relative, so they need no write. Rotation is
   unchanged.
5. **Fit to content (AC 22–27).** `ui-core` computes the content box as the
   union of `object_outline_bounds` over all objects: curve extremes, rotated
   primitive outlines, no stroke width. It does not use `object_bounds`,
   which is a primitive's unrotated frame and would break AC 23 for rotated
   shapes. `document-core` cannot compute curve bounds, because
   `geometry-core` depends on it. `Document::fit_to_content((Point, Point))`
   sets each axis to `max(extent, MIN_DOCUMENT_MM)` and moves every object by
   `−min`, plus `(MIN − extent)/2` on an axis narrower than the minimum
   (AC 25). It commits once, labelled `fit_document_to_content`. If the size
   and the move are both zero within 1e-9 mm, it writes nothing (AC 26).
   With no objects, the panel does not render the button (0017 criterion 3:
   a control is not rendered rather than disabled). A content box wider than
   `MAX_DOCUMENT_MM` is refused with the same typed error and the panel's
   validation message (flag 4).
6. **Limits and tolerances.** `MIN_DOCUMENT_MM = 1.0` and
   `MAX_DOCUMENT_MM = 100_000.0` are public constants in `document-core`
   (AC 16, Question 4). A typed value converted to mm is accepted within 1e-9
   mm of either bound and then clamped to it, so 0.1 cm and boundary inch
   values are not refused by rounding. Equality and idempotence use 1e-9 mm,
   the same as `MOVE_EQUAL_EPSILON_MM`. Resize and fit check no coordinate
   limit. A resize moves objects by at most 50 000 mm, and a fit moves them
   toward the origin, so no value can become non-finite.
7. **The view follows the shift (AC 20).** After a resize or fit commits,
   `Session` moves the viewport origin by the same shift
   (`Viewport::pan_by_document_offset(Vec2)`, new in `ui-core`). No object
   moves on screen.
8. **Rulers are two Canvas2D strips drawn by the frontend from data that
   `ui-core` computes (AC 1–10).** Other options considered:
   - Drawing in `render-core`/wgpu lost because `render-core` cannot draw
     text. A glyph atlas or a font dependency is too much for tick labels.
   - DOM or React elements per tick lost: about 300 nodes reconciled on
     every pan frame, and React commits asynchronously, so the strip cannot
     update in the same frame as the canvas (AC 2).

   The strips are grid siblings of the canvas (corner, top, left). They
   shrink the canvas element, so the existing `getBoundingClientRect` path
   already makes AC 9 hold. They receive no pointer input that reaches the
   session (AC 10). A `Rulers.tsx` component runs its own
   `requestAnimationFrame` callback, which paints in the same frame as the
   wgpu render. It redraws only when the scale, origin, canvas size, unit or
   pointer changed. It scales its backing store by `devicePixelRatio`. Do
   not add the drawing to `useEditorSession.ts`, which is already 1,571 lines.
9. **The tick algorithm is one pure `ui-core` module, `ruler.rs`, tested by
   cargo (AC 3–7).** Input: `ViewTransform`, the axis, the strip length in
   px, the `DisplayUnit`, and the advance of one digit in px, which the
   frontend measures once with `measureText`. Output: a regular grid
   (`first_major_px`, `major_px`, 5 minors per major, first major index, step
   as mantissa {1, 2, 5} × 10^exp in the unit) and the labels of the visible
   majors. The major step is the smallest 1-2-5 step that is at least 40 px
   and at least the widest visible label plus 4 px. With 12 px digits and
   labels of seven or more characters, 40 px alone cannot meet AC 7
   (flag 2). Labels are built from integers (`index × mantissa` with the
   decimal point placed by `exp`), never by printing an `f64`. That makes
   "0.3" exact and removes trailing zeros (AC 6). Cargo tests cover the AC 5
   examples, AC 6, the AC 2 projection check (every major is within 0.5 px of
   `view.document_to_screen`) and no-overlap across all zooms up to
   ±1,000,000. Rotation of the vertical labels is the ux-engineer's choice.
   The algorithm measures label length along the axis, so it holds either way.
10. **Pointer marker (AC 8).** The frontend draws it at the pointer's
    canvas-relative CSS position, the same position the status-bar readout
    converts. That makes them equal by construction. No Rust is needed.
11. **Document area and pasteboard (AC 28–31).** The GPU clear colour in
    `gpu.rs` becomes the pasteboard colour. `render-core` adds
    `build_document_area(DocumentSize)`: two triangles in `--canvas-bg` in
    document coordinates. `Session::draw_list` prepends it as artwork layer
    0, so it pans and zooms with the content and draws behind everything.
    `build_pen_preview` takes the `DocumentSize` and fills each knockout
    with the canvas or the pasteboard colour, chosen by the knockout centre
    (AC 31). Both colours are constants in `render-core`'s `theme.rs`. The
    duplicate `CANVAS_BACKGROUND` in `gpu.rs` is removed. Tools never read
    the size (AC 29, 30).
12. **The status bar reads the size from the session, not the host
    (AC 12, 21).** `curvyo-app` sends `ProjectStatePayload.size_mm`, which is
    always A4. Its own doc comment says that the slice that adds resizing
    must move it. Remove the field, and remove the event and command too if
    nothing else uses them. The frontend reads `document_view()` (size and
    unit, already formatted by `ui-core`) after every sync.
13. **The panel's Document section, and how it coexists with 0017
    (Question 1 A).** `ui-core` gets one pure function, `panel_content(tool,
    selection) -> PanelContent { Document, Style(scope), Empty }`, next to
    `style_scope`, as an enum with no trait. Document is shown exactly when
    the object selection is empty. Otherwise 0017's rules decide between
    Style and Empty. The frontend renders one of `DocumentSection` or
    `StyleSection` from that value. 0017 criterion 1 ("no heading, no
    control") then applies only to the remaining empty cases, so the PO
    amends it (flag 3). 0017's focus rule is unchanged: when a Style control
    leaves the tree, focus goes to the canvas, not into the Document fields.
    Shift+Ctrl+F reaches Width through the existing `data-first-focus`. New
    code goes into new files: `session/document.rs`, `wasm_document.rs`,
    `DocumentSection.tsx` and `useDocumentPanel.ts`, using the existing
    `NumberField` and the `"committed" | "unchanged" | "invalid:<code>"`
    pattern of `set_style_text`.
14. **Crate boundary.**
    - `document-core`: `DisplayUnit`, size validation on read, `resize`,
      `fit_to_content`, `set_display_unit`, the limits, `DocumentSizeError`,
      the shared move helper, `document.json`.
    - `geometry-core`: no change.
    - `ui-core`: `ruler.rs`, `display_unit_text.rs`, the content box (in
      `object_bounds.rs`), `panel_content`,
      `Viewport::pan_by_document_offset`.
    - `render-core`: document area, pasteboard colour, knockout colour.
    - `editor-wasm`: the new session and wasm modules above, and the clear
      colour.
    - `curvyo-app`: remove `size_mm`.
    - `frontend`: rulers, layout, Document section, status bar.
15. **Three PRs, in this order.**
    1. Model: decisions 1–6 and the `ui-core` parse, format and content box.
       Cargo tests only. No UI.
    2. Rulers and pasteboard: decisions 8–12, with ruler labels in mm. This
       can go before PR 1 if the ruler takes `DisplayUnit` from it. Keep PR 1
       first so the type exists once.
    3. Panel: decisions 7 and 13, the unit control, Fit, and units in the
       status bar. It goes after `style-panel-rework` (0017) if 0017 is
       built first. That is the safer order, because 0017 owns the panel
       frame.
16. **Overlap with `0016-boolean-operations`.** Both features touch
    `document-core`, `ui-core`, `render-core`, `editor-wasm` and `frontend`,
    so CLAUDE.md §4 does not let them be built in parallel. Recommendation:
    build this feature's PR 1 first. It is small and has no format bump. Its
    only seam with booleans is the move helper in `objects.rs`, which
    booleans then extends for compound paths in one place. Resize and fit
    then cover compound paths automatically. Fit reads compound bounds once
    booleans extends `object_outline_bounds` (its AC 34). Whichever of the
    two merges second adds a compound-path case to the resize test (AC 17)
    and the fit test (AC 23). If booleans goes first, this feature only
    rebases. No version number has to change in either order.

## Dated notes from PR 2 (2026-10-09, implementer; the architect confirms)

- **Decision 7 (view):** the default view of a new or opened project is
  `Viewport::with_document_inset` (72 px, AC 11a), applied in
  `WasmSession::new` and `open`, not in `Session::new`, so headless tests keep
  the `0004` view. A flag keeps its origin through window resizes until the
  first pan, zoom or drag-pan; it ends there, and `0004` criterion 10 applies
  again. Browser check: without the flag a window resize of a fresh view moves
  the corner; the early size reports at attach do not. PR 3's
  `pan_by_document_offset` must say in a test whether it ends the flag
  (recommendation: it does not).
- **Decision 9 (ruler):** the step follows AC 5 (label plus 4 px); labels thin
  below label plus 8 px (UX notes), so the end of a label never touches the
  next major tick. The PO rewords AC 7 accordingly. The layout returns an empty
  layout for a non-finite view or strip and for a view that would hold more
  than 4096 majors.
- **Decision 11 (draw list):** the document area is prepended in
  `Session::frame_draw_list`; `Session::draw_list` stays the artwork and
  overlay for headless tests. A future raster, thumbnail or export path must
  use `frame_draw_list`.

## Flagged to the lead and the PO

1. **AC 38:** the display unit needs no `format_version` bump (decision 1).
   Reword: the unit is stored; an older build opens the file in mm and keeps
   the stored unit when it saves.
2. **AC 5 vs AC 7:** a 40 px minimum cannot fit labels of 7 to 9 characters
   ("−999999.8") at 12 px. Add "and at least the widest label plus 4 px".
   The AC 5 examples are unchanged.
3. **0017 criterion 1** needs a matching amendment: with no object
   selected, the panel shows the Document section. With Question 1 B or C,
   decision 13 is dropped and 0017 stands as written.
4. **Missing criteria:** what Fit does when the content is wider than
   100 000 mm (default: refused with the validation message), and the
   status-bar precision per unit (`toFixed(1)` is coarse in inches; the
   ux-engineer decides).
5. **AC 19 and 27** name user-facing labels. The stored commit labels follow
   the code's convention (`resize_document`, `fit_document_to_content`). The
   undo slice maps them to the visible text.
6. With collaboration (post-MVP), a per-document unit is shared: one
   collaborator switching to inches switches everyone (last writer wins).
   This is accepted with Question 2's per-document default. A per-user unit
   would need an app settings store, which does not exist yet.
