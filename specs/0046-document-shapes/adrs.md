# ADRs for "Document shapes" (sketch)

A document-model and file-format change: **`needs-customer`**. These notes
fix the direction so that 0045 and 0040 do not block it. A short ADR (the
next free number, `needs-customer`) is written when stage 1 is scheduled, not
before. Reference state: `main` at `52d4101`.

## Depends on

- [ADR 0002 §2, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  mm only, one commit per edit; SVG user units at 96 per inch (stage 2).
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  an older build must refuse the file rather than draw a rectangle where a
  key ring is, so the version goes up.
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md):
  last-writer-wins registers in the document root.
- [`0040` adrs.md](../0040-document-background/adrs.md) decisions 2, 6 and 7:
  the root-register pattern, `build_document_area` with its checkerboard
  prefix, and `background_at` containment.
- [`0045` adrs.md](../0045-document-formats-library/adrs.md) decision 9: a
  format gains a `shape` field and the next schema numbers.
- [`0016`](../0016-boolean-operations/) compound path model and
  `subpath_codec.rs` for the outline.

## Notes (2026-10-10)

1. **Register (Question 1, `needs-customer`, recommended A).** Three root
   keys, each a last-writer-wins register like 0040's:
   - `page_shape`: `"rounded" | "ellipse" | "outline"`. Absent means
     Rectangle, so every existing file opens unchanged and opening writes
     nothing (criterion 1).
   - `page_corner_radius_mm`: an f64, clamped on read to half the shorter
     side. It is not rewritten by a resize.
   - `page_outline`: one Loro list value (replaced whole, never mixed across
     peers) of subpaths in the `subpath_codec` encoding.
2. **The outline is stored normalised to the unit box [0, 1]²; the size
   registers stay the bounding box.** A resize then writes only width and
   height, so 0015's resize, Fit and 0030/0045 picks stay unchanged. A
   concurrent resize and shape change merge cleanly. The outline cannot
   drift away from its box (`docs/technical-debt.md`, "Resize and fit merge
   per field across peers"). The proportion lock of criterion 12 is a rule
   of the resize command for Outline only. Rejected: the outline in mm.
   Every resize would rewrite it, and a peer's resize would leave box and
   outline disagreeing.
3. **Bounding box semantics (criteria 2, 3).** `DocumentSize` stays the box.
   Rulers, the status bar, the origin and "resize about the centre" are
   unchanged. Fit to content applies to Rectangle and Rounded only
   (criterion 13).
4. **Format: `CURRENT_FORMAT_VERSION + 1`, next free at merge** (after 0040
   and 0023). Golden fixtures: one file per shape, one damaged file per
   validation rule (unknown kind, radius not finite, outline that is not
   closed or is over 2,000 anchors), and old files opening as Rectangle.
5. **Drawing.** `render-core::build_document_area` takes the shape:
   - Rectangle keeps today's quad, so 0040's tests are unchanged.
   - Other shapes are filled with `lyon` (already a dependency), non-zero
     rule from the compound path, flattened to 0.01 mm at 100 %. 0040's
     fragment checkerboard works on any geometry. Holes show the pasteboard
     (the clear colour). The edge is the document-edge stroke along the
     outline.
   - The tessellation is cached by (document version, scale), the draw-list
     cache of 0044 decision 5.
   - `background_at` and the eyedropper use a point-in-shape test from
     `geometry-core` instead of `DocumentSize::contains`. No clipping of
     objects (criterion 6).
6. **Importers (stages 2, 3).** Stage 2 reuses 0024's SVG importer (a core
   crate, golden files). Stage 3 needs its own DXF spec. Whether it uses a
   DXF crate or our own reader, and in which crate it lives, is decided in
   that spec's `adrs.md`; a new crate needs an ADR (`CLAUDE.md` §5). The
   platform reads the file, and core gets bytes (criterion 19).
7. **Library entries (criterion 15).** In the formats files the outline is a
   path `d` string in mm, as the spec says (readable by hand). Picking a
   format normalises it into the register of note 1. A pick writes size and
   shape in one `resize_document` commit, which extends `resize_document_to`.

## Open for the customer (with the PO's questions)

- Question 1 (model): A recommended. Default if unanswered: the slice waits;
  it is not built without an answer, because it changes the file format.
- Question 3 (DXF timing): default A, SVG first.
