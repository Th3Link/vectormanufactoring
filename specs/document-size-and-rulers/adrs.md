# ADRs for "Document size, rulers and multiple pages"

This is the largest document-model change since `project-file-foundation`.
The storage decision is
[ADR 0012](../../docs/adr/0012-pages-in-the-document-model.md), which is
**Proposed and needs the customer**. Everything else fits existing crates over
existing edges. **No new crate, no new dependency, one `format_version` bump.**
AC 8 conflicts with AC 6 as written (see "Flagged to the lead", 1).

## Depends on

- [ADR 0012](../../docs/adr/0012-pages-in-the-document-model.md): pages are
  the root nodes of the object tree, with per-page size, page-local
  coordinates, a `PageId`, at least one page, an ephemeral active page, and
  migration on open. AC 13–19 are built on it.
- [ADR 0002 §2, §4, §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  mm, Y-down, page origin at top-left (AC 2), and `NodeId`s that survive the
  migration.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  resize, fit, add page and remove page are each **one commit** (AC 5, 17).
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): the
  active page, like the view and the selection, is never written.
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md): page
  width and height are separate last-writer-wins registers.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  older files open and are migrated, and a newer file is refused.
- [ADR 0001 §4](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the page rectangle is part of the WebGL draw list.
- [`specs/0004-canvas-navigation-and-selection/adrs.md`](../0004-canvas-navigation-and-selection/adrs.md):
  `viewport` in `ui-core`, "all screen↔document conversion happens in Rust",
  "one bounds rule" (`object_bounds`), `translate_objects`'s per-kind
  translation, and "a draw list depends on the view's scale, never on its
  origin".
- [`specs/0001-project-file-foundation/adrs.md`](../0001-project-file-foundation/adrs.md):
  its "minimal document root record" is replaced in shape by ADR 0012. The
  default size of 210 × 297 mm is still the size of every new page (AC 16).

## Feature-local decisions

- **2026-10-05: `format_version` is provisional.** `main` writes 3 today.
  `path-merge-split-and-node-types`, `object-transform` and
  `stroke-and-fill-styling` currently claim 4, 5 and 6. This slice takes
  `main`'s `CURRENT_FORMAT_VERSION + 1` at the moment its PR merges, and
  renumbers its fixtures and this note on rebase (rule from
  `specs/0006-path-merge-split-and-node-types/adrs.md`). The migration from
  every older version is ADR 0012 §6. It runs in `document-core`'s open path,
  keyed on the manifest's `format_version`: first validate the old shape, then
  migrate, then validate the new shape. Fixtures: an old-version file opens
  as one page with unchanged `NodeId`s and object order. Also a two-page
  round trip, and a zero-page snapshot that opens with one default page.
- **2026-10-05: tree validation on open.** Every root must be a valid page
  (finite, positive `width_mm` and `height_mm`). Every child of a page is
  validated as an object, exactly as roots are validated today. Deeper
  nesting is refused as damaged until groups exist. **Required test:** an
  object under a deleted page counts as nonexistent (`node_exists`,
  `translate_objects`, a lazy selection resolve). Loro hides a deleted node's
  children rather than deleting them, so this needs a test, not an
  assumption.
- **2026-10-05: `document.json` becomes
  `pages: [{ width_mm, height_mm, objects: [...] }]`** in page order. The
  root `size` disappears. Golden fixtures are regenerated.
- **2026-10-05: page size is validated.** `resize_page` refuses a non-finite
  value and any dimension below `MIN_PAGE_MM = 1.0` with a typed error. The
  Properties fields reject such input before they call it. There is no upper
  bound.
- **2026-10-05: center-anchored resize is one `document-core` command.**
  `Document::resize_page(PageId, DocumentSize)` computes
  `shift = ((w₁ − w₀)/2, (h₁ − h₀)/2)`. In **one commit** it writes both size
  registers and translates every object on the page by `shift`, using the
  same per-kind translation `translate_objects` uses. That translation is
  extracted into a shared private helper, not copied. Primitives move their
  frame origin or centre, and rotation (slice 5) is unaffected. An empty page
  changes only its size. This is elementary arithmetic on `document-core`'s
  own types, so `geometry-core` is not involved (AC 5–7).
- **2026-10-05: fit to content sets the page to the content box. It does not
  apply AC 6's rule** (see flag 1). `ui-core` takes the union of
  `object_bounds` over the page's snapshots. This is the same function that
  draws the selection boxes, so curve extrema come from `geometry-core`'s
  `segment_bounds` and nothing is reimplemented.
  `Document::fit_page(PageId, content: (Point, Point))` (min, max corners, the type `object_bounds` already returns) sets the size to the box's
  extent and translates every object by `−min`, in one commit, using
  the same helper as `resize_page`. `document-core` cannot compute curve
  bounds itself because `geometry-core` depends on it. A box narrower than
  `MIN_PAGE_MM` on one axis grows to that minimum, and the content is centered
  on that axis. If the page has no objects, the action is disabled (AC 9).
  Bounds are geometric and ignore stroke width (flag 3).
- **2026-10-05: the view follows the shift.** After `resize_page` or
  `fit_page`, `Session` moves the viewport origin by the same `shift`. The
  content does not move on screen and the page edges move around it. This is
  what AC 8's "exactly where it was on screen" requires. For a resize it
  makes "content stays centered" visible as the page growing or shrinking
  around the content. The `ux-engineer` may choose otherwise for resize
  alone.
- **2026-10-05: the active page lives in the editor `Session`** (a
  `session/pages.rs` glue module in `vecmanf-editor-wasm`, the same pattern
  as `session/select.rs`). It holds one `PageId`. Render and hit-test
  snapshots come from `Document::objects_on(page)`, so no other page is
  visible or selectable by construction (AC 15). A page switch clears
  `ObjectSelection` and `NodeSelection` and cancels any tool's in-flight
  state, because those ids still exist and would not drop on a lazy resolve.
  The view is unchanged by a switch. Zoom presets remain out of scope. Add
  page appends a root after the last page, switches to it, and is one commit
  (AC 16). Remove page is one tree delete, then switches to the next page,
  or to the previous one if there is no next page (AC 17). The status-bar size reads
  the active page (AC 19).
- **2026-10-05: rulers are computed in `ui-core` and drawn by the frontend.
  They are not part of `render-core`.** They have no document-model impact.
  `render-core` cannot draw text, and ruler labels are text. A `ruler`
  module in `vecmanf-ui-core` has one job: tick positions for one viewport
  axis. Input is the `Viewport` and the canvas length in px. Output is a
  list of (screen px, value mm, major or minor). The major step is the
  smallest value in {1, 2, 5} × 10ⁿ mm whose on-screen length is at least
  40 px. This always lands in [40, 100) px (AC 4). Examples: 20 mm at 100 %,
  0.2 mm at 8000 %, 1000 mm at 2 %. Each major step has five minor
  subdivisions. Because each page's origin is its own document origin, 0 mm
  is the page corner by construction (AC 2), and negative values and values
  past the page size need no special case (AC 3). The wasm API returns the
  ticks already in pixels (slice 4's rule). The frontend draws two strips,
  using Canvas2D or the DOM, and handles `devicePixelRatio`.
- **2026-10-05: the pasteboard is drawn by `render-core`. Nothing is stored
  for it.** The GPU clear colour becomes a new `--pasteboard-bg` token, which
  the `ux-engineer` defines in `docs/design-system.md`. The first draw-list
  item is the active page's rectangle `(0, 0)–(w, h)`, filled with
  `--canvas-bg` in document coordinates. It therefore follows pan and zoom and
  depends only on scale (AC 10). Tools never consult the page bounds
  (AC 11–12). `pen_preview.rs` fills its knockouts with `CANVAS_BG`, so they
  will show as light dots on the pasteboard. That is a `ux-engineer` check.
- **2026-10-05: the crate boundary.**
  - `vecmanf-document-core`: page storage, `PageId`, `pages()`,
    `page_size`, `objects_on`, `add_page`, `remove_page`, `resize_page`,
    `fit_page`, object creation under a page, migration, zero-page repair,
    `document.json`, validation, `MIN_PAGE_MM`.
  - `vecmanf-geometry-core`: no change.
  - `vecmanf-ui-core`: `ruler`, and the content-box union over `object_bounds`.
  - `vecmanf-render-core`: the page rectangle and the pasteboard colour.
  - `vecmanf-editor-wasm`: `session/pages.rs`, the ruler tick export, and the
    clear colour in `gpu.rs`.
  - `frontend/`: the rulers, the Pages section, the page-size fields, Fit to
    content, and the status bar.

## Flagged to the lead

1. **AC 8 contradicts AC 6.** Under AC 6, the old page's center maps to the
   new page's center. After a fit, the content box fills the new page exactly
   only if the content was already centered on the old page. In every other
   case some content ends up outside the fitted page. Default: the decision
   above (translate by `−min`, view follows). The PO should reword
   AC 8 to say that the fitted page is centered on the content, not on the
   old page.
2. **ADR 0012 is needs-customer.** Two points the customer should confirm:
   (a) the PO's reading of the request as switching between pages, not
   placing them side by side (ADR 0012 §2 keeps side by side additive);
   (b) the active page is not saved, so reopening a project shows page 1.
   Default if there is no answer: accept as proposed and build.
3. **Fit uses geometric bounds.** After `stroke-and-fill-styling`, a wide
   stroke will extend past a tight page by half its width. This is right for
   cut lines and wrong for artwork that is printed as drawn. Default:
   geometric. The PO may add a visual-bounds option later.
4. **Gaps the PO should add as criteria:** the 1 mm minimum page size and
   what invalid size input does, and that a page switch clears the selection
   and keeps the view.
5. **No conflict with any accepted ADR.** The specification is not Ready yet.
   It needs UX notes, the AC 8 rewording and an answer on ADR 0012.

---

**2026-10-05, PO note (not an ADR edit):** the customer dropped multi-page
scope for MVP; `specification.md` in this folder (moved from
`specs/document-size-rulers-and-pages/`) no longer asks for pages at all.
Everything above that depends on ADR 0012 or "page" as a document-model
concept is stale and needs an architect pass — flagged here only so the
next reader does not treat this file as current; rewriting it is the
architect's call, not the PO's.
