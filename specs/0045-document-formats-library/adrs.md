# ADRs for "Document formats library"

The 0030 preset list becomes a library: the built-in file plus an optional
user file, both parsed and validated in `curvyo-document-core`, read and
written by the platform. **No new ADR, no new crate, no trait, no generic, no
project format bump.** It adds one **new file format we write** (the user
file, `format = 1`) and turns on the `toml` writer. Reference state: `main` at
`52d4101`.

## Depends on

- [ADR 0004 §7, §8, §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  data directory, atomic write, no lock files, folder sync. Merge rules
  (import) are pure functions in core. Every written file carries a format
  version; a newer one is refused, never partially read.
- [ADR 0011 §2, §3](../../docs/adr/0011-workspace-and-crate-layout.md):
  `curvyo-storage-io` does the file system and `curvyo-app` the paths and
  dialogs; core opens no file and builds for `wasm32`.
- [ADR 0001 §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  views and form rules in Rust; the frontend renders them and holds typed
  text only.
- [`0030` adrs.md](../0030-document-size-presets/adrs.md): `document_presets.rs`
  (model, validator, `PRESET_MATCH_TOLERANCE`), `document_presets_view.rs`
  (pick rule, strips), `resize_document_to` (one commit per pick).
- [`0043` adrs.md](../0043-properties-tabs/adrs.md): the Document tab hosts
  the section. Criterion 23's "section hidden" is the section unmounting.

## Feature-local decisions (2026-10-10)

1. **Where the user file lives (Question 1): `needs-customer`, default A.**
   The file is `document-formats.toml` in the **data directory** (ADR 0004
   §7). Until the configurable data directory exists, that is Tauri's
   `app_data_dir()` (on Linux `$XDG_DATA_HOME/<identifier>/`); when the
   setting comes, only the base path changes. Not the config directory: that
   holds machine-local settings that should not follow a synced folder,
   while formats are the maker's data and should. Not the project (option
   B): the document model and the CRDT would carry settings, and every
   project would have its own stars.
2. **Model: three modules in `curvyo-document-core`, next to `document_presets.rs`.**
   - `document_presets.rs` (443 lines) reads built-in schema 2: group
     `enabled`, preset `favourite`, unit `cm` (`PresetUnit::Cm`, `* 10`).
     The 0030 validator (`PresetReason`) applies to every built-in and user
     entry.
   - `format_library.rs` owns the merged library and its edits. It holds the
     built-in list, the user groups (new or appended to a built-in id), the
     favourite ids and the per-group on/off overrides. It applies the overlay
     rules of criterion 4: structure errors strict, unknown ids in
     `favourites`/`enabled` ignored. It also checks the limits of criterion 5
     (200 formats, 20 groups, 256 KiB checked on the text before parsing) and
     same-size across the whole library within `PRESET_MATCH_TOLERANCE`.
     Edits: `add_format`, `edit_format`, `delete_format`, `rename_group`,
     `delete_group`, `set_favourite`, `set_group_enabled`, `import(text) ->
     ImportReport` (criterion 27 as a pure merge, ADR 0004 §8), `export_text()`.
     Each edit validates against the whole library and returns
     `Result<_, FormatError { field, reason }>`.
   - `format_library_file.rs` reads and writes the user file. Serde structs
     with `deny_unknown_fields`, `format = 1`, and refusal of any other value
     with "made by a newer version". The writer emits a canonical order
     (`favourites`, `enabled`, then groups in creation order), so
     read-write-read is the identity. Tested with golden fixtures in
     `curvyo-document-core/tests/fixtures/formats/`: one valid file, one per
     failing rule, one for the round trip, the export, and the import merge.
   - "Favourite" when the user file has no `favourites` key: the built-in
     `favourite` flags. Once the key exists, it is the whole list.
3. **Ids.** Built-in ids never start with `u-` (a test on the shipped file).
   A new or imported format gets `u-<slug of name>`, with `-2`, `-3` on a
   collision. That is pure and deterministic: no clock, no random number.
   Group ids work the same way.
4. **Dependency: the `toml` writer.** The workspace `toml` gains the
   `display` feature. It pulls `toml_writer` 1.1 (MIT OR Apache-2.0), which
   is **already in `Cargo.lock`** through `toml` 1.1, so nothing new is
   downloaded. It is pure Rust and builds for `wasm32`. The workspace comment
   "parser only, no writer" changes. Rejected: a hand-written emitter (string
   escaping is where it goes wrong); `toml_edit` to keep the maker's comments
   (a larger API for a file the app owns).
5. **I/O split.**
   - `curvyo-storage-io` gains `rename_replacing(from, to)` for "Set file
     aside" (criterion 6). Reading and writing use the existing `read_to_vec`
     and `write_atomic` (temporary file plus rename).
   - `curvyo-app` gets Tauri commands that move text only, with no parsing
     and no `document-core` edge: `read_formats_file() -> Option<String>`
     (`None` = missing), `write_formats_file(text)`, `set_formats_file_aside()`,
     and `import_formats_file()` / `export_formats_file(text)` through the
     existing `tauri-plugin-dialog`.
   - **Browser (demo build):** `frontend/src/platform/host.ts`
     `browserCommands` serve the same names. The file text sits in
     `localStorage` under `curvyo.document-formats` (each access in
     try/catch; a failure means built-ins only). Import is a file picker and
     export a download, as Open and Save are today. Core is unchanged
     (criterion 31).
6. **Runtime state in `Session`.** `FormatLibrary` replaces the `PresetList`
   field. `load_formats(text) -> Result<(), String>`: an error keeps the
   built-ins and sets `formats_broken: Option<String>`, and then every edit
   is refused in Rust as well as hidden in the view (criterion 6). Every
   successful edit returns the new user-file text, and the frontend hands it
   to `write_formats_file`. A failed write shows a notice and the in-memory
   library stays; the last writer wins (criterion 30). The frontend reads
   the file before the panel first renders, so the quick selection does not
   flash the defaults.
7. **Views in `curvyo-ui-core`, pure.** `document_presets_view.rs` (380 lines)
   keeps the pick rule and becomes the quick selection: favourites of groups
   that are on, and the subject line over all formats of groups that are on
   (criteria 8, 9, 11). New `format_list_view.rs`: the "All formats" list,
   with group rows, counts, stars, which buttons exist and the accessible
   names (criteria 13, 17). New `format_form.rs`: the prefill (criterion 18:
   the document size in the display unit, never px, the last used group) and
   the field validation with messages (criteria 19 to 21, 24). The frontend
   holds the typed text and whether a form is open. Every rule and message
   comes from Rust. The list and form components go in `panel/`, under
   0017's lint rule (no popover, listbox or menu).
8. **Sync.** Folder sync works now: one file in the data directory, atomic
   write, no lock. Cloud sync of settings is later, with ADR 0004's library
   sync. **Accepted cost:** the file is one file, not one per record (ADR
   0004 §6), so two machines that edit formats at the same time leave a
   conflict copy, and ADR 0004 §7's detection reports it once it exists.
   Revisit (one file per group) if settings ever merge automatically.
9. **0046 hook, nothing built:** a format entry is a struct; 0046 adds a
   `shape` field and the next schema numbers (built-in 3, user 2).
10. **Project format: no change** (criterion 28). A project stores millimetres
    only.

## Delivery: one branch `story/document-formats-library`, one PR

1. `document-core`: schema 2, the library, the user file, edits,
   import/export, fixtures (criteria 1 to 7, 26 to 28). Touches only
   `document-core` and the workspace `Cargo.toml`, so it **may start now, in
   parallel with 0043**. The architect reviews this milestone's diff, because
   it is a new written file format.
2. `storage-io`, `app` commands, the `host.ts` fallback, the session state,
   the `ui-core` views and form rules, and session tests (criteria 8 to 11,
   14 to 16, 20 to 24, 29, 30).
3. Frontend: the list, stars, switches, inline forms, import and export
   (criteria 12 to 25). Then the UX review and the tester pass.

Milestones 2 and 3 start after 0043 and 0040 merge: they share
`DocumentSection.tsx`, `useDocumentPanel.ts`, `session/document*.rs`,
`wasm_document.rs`.

## Flagged to the PO (defaults taken)

1. The app rewrites the whole user file on every change, so comments a maker
   typed by hand are lost (decision 4). The built-in file keeps its
   comments.
2. Criterion 31 ("not built here"): the browser fallback costs about ten
   lines in `host.ts` and keeps the public demo working. Built (decision 5).

## Notes added during the build (2026-10-10)

- Built-in group ids are never removed, because a user file may append formats to one (collision rule
  between a later built-in format and a maker's format: `docs/technical-debt.md`, "The user formats file
  depends on the built-in list").
- An empty or blank user file means "no formats of the maker's", not the broken-file state.
- An import joins a group of the maker's by name (ignoring case) or any group by id; a foreign id (one that
  does not start with `u-`) is replaced by a generated `u-` id.

