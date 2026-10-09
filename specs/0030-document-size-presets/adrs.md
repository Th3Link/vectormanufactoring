# ADRs for "Document size presets"

A preset press is a resize as `0015` defines it, and the preset list is
read-only data compiled into a core crate. **No new crate, no new ADR, no
`format_version` bump, no new document register, no trait, no generic.** One
new dependency edge: `toml` into `curvyo-document-core`; the package is
already in `Cargo.lock`. Reference state: `main` at `909a2cc`.

## Depends on

- [ADR 0002 §2, §3](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  the document stores mm only; a preset side is a `Length` after loading.
  "Same size" comparisons take an explicit tolerance (0.01 mm, criterion 10).
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  a pick or an orientation swap is one `resize_document` commit (criteria 11,
  14).
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  nothing new is stored, so no bump (criteria 19 to 21).
- [ADR 0001 §1, §3, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the pick rule, the selected state and the tooltip text are pure `ui-core`
  functions; `editor-wasm` passes them through; the frontend renders them.
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md) and
  `CLAUDE.md` §6: a core crate does no I/O and builds for `wasm32`.
- [`specs/0015-document-size-and-rulers/adrs.md`](../0015-document-size-and-rulers/adrs.md)
  decisions 3 (units are types; inches as `* 254 / 10`), 4 (`Document::resize`
  keeps the centre, `Ok(false)` within 1e-9 mm), 6 (the limits), 7 (the view
  follows the shift), 13 (`panel_content`, `DocumentSection.tsx`).
- [`specs/0017-style-panel-rework/adrs.md`](../0017-style-panel-rework/adrs.md)
  decision 6: the panel folder's lint rule forbids popover, select and menu
  imports; the inline `ToggleGroup` is allowed.

## Feature-local decisions

- **2026-10-10: data file. TOML, at `curvyo-document-core/data/document-presets.toml`,
  compiled in with `include_str!`.** The format is the spec's "Data file
  format" section, unchanged (`format = 1`, `[[group]]`,
  `[[group.preset]]`, `short_side`/`long_side`/`unit`, optional `note`).
  - Parser: `toml` (MIT OR Apache-2.0) as a workspace dependency, with
    default features off and only `std`, `parse` and `serde` on (no writer).
    The implementer checks these feature names against the pinned version.
    `toml` 1.1 and its parts (`toml_parser`, `winnow`, `serde_spanned`,
    `toml_datetime`) are already in `Cargo.lock` through the Tauri build, so
    no new package is downloaded and `cargo deny` already covers them. All of
    them are pure Rust and build for `wasm32-unknown-unknown`. The PR states
    the dependency, licence and reason (`CLAUDE.md` §3).
  - Rejected: **JSON through `serde_json`**: no new edge, but no comments, and
    the file is meant to be edited by hand (Question 3, default TOML).
    **RON**: a new package, and few makers know it. **A Rust table**:
    criterion 2 forbids it. **A build script that generates Rust**:
    criterion 7 forbids it, and a broken file would fail as a compiler error
    in generated code, not as a test that names the preset. **A platform
    crate reading the file at run time**: it needs a file path in Tauri and a
    fetch in the browser, for a file that ships with the binary anyway.
  - The file is not a file format we write. User presets later
    (out of scope) would be a second text handed to the same loader, so
    `format = 1` is the only compatibility promise we make now.

- **2026-10-10: model and loader in a new module
  `curvyo-document-core/src/document_presets.rs`.** It belongs next to
  `DocumentSize`, `MIN_DOCUMENT_MM`/`MAX_DOCUMENT_MM` and `Length`, which the
  validation needs (criterion 6f). `ui-core` would also work, but it would
  then import the limits and duplicate the unit conversion.
  - Public types: `PresetList { groups: Vec<PresetGroup> }` (`Default` =
    empty), `PresetGroup { id, name, default_orientation: Orientation,
    presets: Vec<DocumentPreset> }`, `DocumentPreset { id, name, note:
    Option<String>, short_side: Length, long_side: Length, authored:
    AuthoredSize }`, `AuthoredSize { short: f64, long: f64, unit: PresetUnit }`,
    `enum PresetUnit { Mm, In, Px }`, `enum Orientation { Portrait,
    Landscape }`. `AuthoredSize` holds the numbers as written, only for the
    tooltip (criterion 17: "1920 x 1080 px"); every comparison and every
    write uses the `Length` fields. This is the one bare `f64` pair, the same
    exception `0015` decision 3 makes for a value in its display unit.
    `PresetUnit` is not `DisplayUnit`: px is an authoring unit only, and cm
    is not allowed in the file.
  - `PresetList::parse(text: &str) -> Result<PresetList, PresetError>` is the
    loader of criterion 4. Private serde structs with
    `#[serde(deny_unknown_fields)]` (criterion 6a), then one validation pass
    in file order (criteria 6b to 6h, 8). Conversion: mm as is; in through
    `Length::from_unit(v, DisplayUnit::In)` (so it is the same `* 254 / 10`);
    px as `v * 254 / 960` in one private function. `PresetError { subject:
    PresetSubject, reason: PresetReason }` (`thiserror`), where `PresetSubject`
    is `File`, `Group(String)` or `Preset(String)` and `PresetReason` has one
    variant per rule of criterion 6. A syntax error carries `toml`'s message
    as a `String`; no `toml` type crosses the API.
  - `PresetList::shipped()` is `parse(include_str!("../data/document-presets.toml"))`.
  - `PresetList::matching(size: DocumentSize) -> Option<&DocumentPreset>`:
    the document's shorter and longer sides equal the preset's within
    `PRESET_MATCH_TOLERANCE` (a `Tolerance` of 0.01 mm, a public constant).
    Rule 6h uses the same constant, so at most one preset can match.
    `Orientation::of(size) -> Option<Orientation>`: `None` for a square
    within the same tolerance (criterion 14).
  - If the module passes 500 lines, the validation moves to
    `document_presets_validation.rs`.

- **2026-10-10: validation runs at test time only (criteria 6, 7).** Tests go
  in `curvyo-document-core/tests/document_presets.rs`:
  `the_shipped_presets_load` (criterion 7), `the_shipped_presets_are_the_table`
  (criterion 1, every side within 1e-9 mm), the px and in conversions
  (criterion 5), shipped text plus one extra block (criterion 3), and one
  failing fixture per rule under
  `curvyo-document-core/tests/fixtures/document_presets/` (criterion 6a to
  6h). The test asserts the subject and the reason, not the message text.
  At run time, `Session` calls `PresetList::shipped()` once in `new` and in
  `open`, and keeps `unwrap_or_default()` with a comment that the tests make
  the error branch unreachable. No panic, no `expect`.

- **2026-10-10: pick rule and panel view are pure `ui-core` functions, in a new
  module `curvyo-ui-core/src/document_presets_view.rs`.**
  - `preset_pick_size(list, current: DocumentSize, id) -> Option<DocumentSize>`
    implements criterion 13. If the current size matches a preset of the same
    group, it keeps the current orientation (a square counts as portrait).
    Otherwise it uses the group's default orientation. An unknown id gives
    `None`.
  - `orientation_swap(current, wanted: Orientation) -> Option<DocumentSize>`
    swaps width and height. It returns `None` when `wanted` is already the
    orientation or the document is square (criterion 14: writes nothing).
  - `presets_view(list, size, unit: DisplayUnit) -> PresetsView` lists the
    groups and presets in file order (criterion 8). Each entry carries
    `id`, `name`, `pressed`, `accessible_name` ("Paper A4", criterion 16) and
    `tooltip` (criterion 17, formatted by `display_unit_text`; px presets name
    the authored px first). The view also carries the pressed orientation
    (`Option<Orientation>`). No number or name of a preset is written in
    TypeScript (criterion 2).
- **2026-10-10: interplay with `0015`. A pick is the typed resize, the same
  code path.** `session/document.rs` (409 lines) gets one private
  `resize_document_to(size) -> SizeOutcome`. The body of `set_document_side`
  that calls `Document::resize` and `follow_document_shift` moves into it,
  and both callers use it. So a pick keeps the centre (criterion 11), moves
  the view by `Document::resize_shift` (`0015` decision 7), writes nothing
  within 1e-9 mm (criterion 12, from `Document::resize` returning `Ok(false)`),
  and is ignored while the Pen has an unfinished path (the existing
  `pen_path_unfinished` guard, criterion 18). The new glue goes in `session/document_presets.rs`
  (`apply_document_preset(id)`, `set_document_orientation(o)`,
  `document_presets_view()`), and three pass-throughs go in `wasm_document.rs`.
  The frontend reads the view in the same sync that reads `document_view()`.
  That keeps the selected state in the same frame as the fields
  (criterion 10).
- **2026-10-10: frontend constraints from the "no popups" rule.** Add
  `DocumentPresets.tsx` in `frontend/src/components/panel/` (0017's folder,
  under its lint rule). `DocumentSection.tsx` renders it, and it renders only
  the view. Each group is an inline `ToggleGroup` (Radix `RadioGroup`, the
  same as the Unit control). That gives one Tab stop per group and arrow keys
  that select (criterion 16; see the technical-debt item "Check the arrow keys
  of the radio groups in the real Tauri window"). A `RadioGroup` fires no
  change on its already-checked item, but criterion 12 needs that press to
  reach Rust. So every item also forwards `onClick`, and Rust decides whether
  anything is written. Typed text in Width or Height is committed before the
  press, in the same order the Unit control already uses (criterion 15).
  Tooltips use `ui/tooltip.tsx` (text only). Rows and sizes: UX notes.
- **2026-10-10: format impact: none.** The project file holds no preset id
  and no orientation (criterion 19). Opening a file writes nothing
  (criterion 20). `format_version` does not change.
- **2026-10-10: delivery. The last milestone of the `0017` branch.** The
  customer folds 0030 into the `story/style-panel-rework` PR. 0017 owns the
  panel frame, the `panel/` folder, `EntryField` and `panel_content`, so
  0030 builds on 0017's final panel and does not run in parallel with it.
  Milestones, each leaving the gate green:
  1. `document-core`: data file, `document_presets.rs`, the workspace
     dependency, tests and fixtures (criteria 1 to 8).
  2. `ui-core` + `editor-wasm`: `document_presets_view.rs`, the extracted
     `resize_document_to`, `session/document_presets.rs`, the bindings, and
     session tests for criteria 10 to 14, 18 to 21 (the numbers of
     criteria 11, 13, 14 as a table).
  3. Frontend: `DocumentPresets.tsx`, criteria 9, 15 to 17. Then the UX
     review and the tester pass run on the whole 0017 slice.

## Files shared with other slices

- **0017** (same branch, built before this milestone): `DocumentSection.tsx`
  and `PropertiesPanel.tsx` (moved to `panel/` by 0017), `useDocumentPanel.ts`,
  `session/document.rs`, `wasm_document.rs`, the `lib.rs` export lists of
  `document-core` and `ui-core`, `Cargo.toml`. No overlap with 0017's style
  codec, the `render-core` gradient removal, `ui-core` style modules or
  `session/style*.rs`.
- **0031**: only the `lib.rs` export lists of `document-core` and `ui-core`.
  These are rebase-level conflicts. 0031 starts after 0017 merges.

## Flagged to the PO (defaults taken)

1. Criterion 3, last clause ("the panel component rendered with 9 paper
   presets shows 9 buttons"): the frontend has no test runner. Default: a
   `ui-core` test that the view has 9 entries, plus the tester checks the
   rendered panel in the browser harness.
2. Criterion 13 says "a square counts as portrait", and criterion 14 shows no
   pressed item for a square. Both are built as written; they do not
   conflict.
