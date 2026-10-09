# Plan for Document size and rulers

Three PRs, in the architect's order (`adrs.md`, decision 15). PR 1 (model, no
UI) is merged (#67). PR 2 (rulers and pasteboard) is branch
`story/rulers-and-pasteboard`. PR 3 (panel) gets its own branch from the
then-current `main`. PRs 2 and 3 are staggered with the later PRs of
`0016-boolean-operations` (both touch `ui-core`, `render-core`, `editor-wasm`
and `frontend`).

## Affected crates/modules

PR 1 (model, cargo tests only, no format bump, no UI):

- `curvyo-document-core`: new `display_unit.rs` (`DisplayUnit`,
  `Length::from_unit`, `Length::in_unit`); new `document_size.rs` (limits,
  `DocumentSizeError`, `Document::resize`, `Document::fit_to_content`,
  `Document::display_unit`, `Document::set_display_unit`); `document.rs`
  (`size()` validation on read, root register `display_unit`, `document.json`
  gains `display_unit`); `objects.rs` (one shared per-kind move helper used by
  translate, duplicate, resize and fit); new golden fixtures.
- `curvyo-ui-core`: new `display_unit_text.rs` (parse and format of lengths
  per unit, limits message); `object_bounds.rs` (`content_bounds` for fit).
- Not touched: `curvyo-geometry-core`, `render-core`, `editor-wasm`,
  `curvyo-app`, `frontend`.

PR 2 (rulers and pasteboard): `ui-core` `ruler.rs`; `render-core`
document area and knockout colour; `editor-wasm` view data and clear colour;
`curvyo-app` removes `size_mm`; `frontend` ruler strips, layout, status bar
size readout.

PR 3 (panel, later): `ui-core` `panel_content`, `Viewport::pan_by_document_offset`;
`editor-wasm` `session/document.rs`, `wasm_document.rs`; `frontend`
`DocumentSection.tsx`, `useDocumentPanel.ts`, unit control, Fit, status bar units.

## Tasks

### PR 1: model

- [x] 1. `DisplayUnit` { Mm, Cm, In } with `mm_per_unit()` (1, 10, exactly
  25.4), symbol and lookup, `Length::from_unit` and `Length::in_unit`
  (fulfils AC 33, 35).
- [x] 2. Limits `MIN_DOCUMENT_MM`, `MAX_DOCUMENT_MM`, `DocumentSizeError`, and
  `Document::size()` returning A4 for both axes when either value is missing,
  not finite, or out of range; opening writes nothing (AC 13).
- [x] 3. Extract the per-kind move into one helper used by
  `translate_objects` and `duplicate_objects` (behaviour unchanged), as the
  base for resize and fit (AC 17, 24).
- [x] 4. `Document::resize`: validates, shifts every object by half the size
  change, one commit `resize_document`, same size writes nothing (AC 17, 18,
  19, 39 at model level).
- [x] 5. `Document::fit_to_content`: bounds to size, top-left to (0, 0), 1 mm
  minimum per axis with centring, one commit `fit_document_to_content`,
  idempotent, refuses content larger than the maximum (AC 23 (model part),
  24, 25, 26, 27, 27a).
- [x] 6. Root register `display_unit` (absent or unknown reads mm),
  `Document::display_unit` and `set_display_unit` (one commit
  `set_display_unit`, moves nothing, same unit writes nothing),
  `document.json` gains `display_unit` (AC 36, 38).
- [x] 7. Golden fixtures: v7 file with damaged size values opens at A4; v7
  file with `display_unit = "in"` round-trips; existing goldens open at
  210 x 297 mm (AC 13, 36, 38, 39).
- [x] 8. `ui-core::display_unit_text`: parse a typed length in a unit into a
  `Length` (decimal point or comma, spaces, no unit text, limits with 1e-9
  tolerance and clamping), format a length per unit (3 decimals mm, 4 cm and
  in, no trailing zeros), status-bar formats (fixed decimals 1, 2, 3), and the
  validation message with limits rounded inward (AC 15, 16, 21, 35).
- [x] 9. `ui-core::content_bounds`: union of `object_outline_bounds` over all
  objects of a document (curve extremes, rotated outlines, no stroke width)
  (AC 23).

### PR 2: rulers and pasteboard

- [x] 10. `ui-core::ruler`: tick algorithm (1-2-5 step, 40 px and widest label
  plus 4 px, 5 minors, integer-built labels, label thinning, free-span
  clipping rule) (AC 3, 4, 5, 6, 7).
- [x] 11. Document area and pasteboard colours in the draw list, knockout
  colour by position (AC 28, 31).
- [x] 12. Frontend ruler strips (Canvas2D, same frame as the canvas), corner,
  pointer marker, viewport shrink, press swallowing, wheel forwarding, 72 px
  initial offset, 800 x 600 layout (AC 1, 2, 8, 9, 10, 10a, 11, 11a). The
  initial offset lives in `Viewport::with_document_inset` and survives the
  host's first size reports until the first pan or zoom.
- [x] 13. Status bar size readout from the session; remove `size_mm` from the
  host (AC 12, 21).
- [x] 14. Drawing and editing on the pasteboard with all tools (AC 29, 30
  verified; no tool reads the size).

### PR 3: panel (later)

- [ ] 15. `panel_content` and `Viewport::pan_by_document_offset` (decision 7
  puts the latter in PR 3, not PR 2); the view
  follows a resize or fit (AC 14a, 20).
- [ ] 16. Session and wasm commands for resize, fit, unit; frontend Document
  section with Width, Height, Unit, Fit, notice and validation chip (AC 14,
  15, 16, 22, 26, 27a, 33, 34, 37). `fit_document_to_content` returns
  `Ok(false)` both for "already fits" and for "no objects": show "Already
  fits the content." only if `content_bounds` is `Some`, never on `Ok(false)`
  alone.
- [ ] 17. Status bar units and decimals; save, close and open round trip
  (AC 21, 34, 36, 39).

## Validation

- PR 1: cargo unit tests written first for every task; integration test
  `tests/document_size.rs` covers AC 13, 17-19, 24-27, 36, 38, 39 through the
  public API, including every object kind rotated and not, and a pack, unpack
  round trip. Golden fixtures in `curvyo-document-core/tests/fixtures`.
- Full gate from `CLAUDE.md` section 7 and every step of
  `.github/workflows/ci.yml` on the head sha before each PR is reported.
- PRs 2 and 3: tester acceptance tests; `ux-engineer` review against
  `docs/design-system.md`; Browser pane check at 800 x 600 and the AC 2
  projection test.
