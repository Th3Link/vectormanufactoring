# Architect review: PR #54 (0007 PR 1 of 4, style model + format version 7)

Head `c3139a4`, base `main` at `bdf4a11`. Read against `specification.md`,
`adrs.md` (including the 2026-10-07 readiness check and the implementer notes),
`plan.md`, `CLAUDE.md` §5 and §6. Checked locally: `cargo build --target
wasm32-unknown-unknown -p curvyo-document-core` passes, `cargo nextest run -p
curvyo-document-core` 498 passed. CI on the head is green on all jobs.

## Verdict

**CHANGES REQUESTED** (one small blocking item, a pure move). The document
model, the format, the register split and the stop model are what `adrs.md`
decided, and I would approve them as they stand.

## Blocking

1. **`curvyo-document-core/src/style_codec.rs`: §5 "modules over ~500 lines
   need a reason in the PR or get split" and "one responsibility per module".**
   The non-test part is 564 lines. The PR body gives no reason. The module doc
   lists five jobs: "keys, absent defaults, reads, writes and open-file
   validation". Fix: move the "Open-file validation (strict)" section (lines
   475 to 563: `key_ok`, `number_in`, `is_unit_number`, `is_color`,
   `is_one_of`, `is_dash`, `is_bool`, `style_is_valid`, `stops_are_valid`,
   `stop_is_valid`) as-is into a new `style_validation.rs`, with a one-sentence
   doc: "Open-file validation of the style keys and the stop list". Make the
   key and name constants it needs `pub(crate)`. That leaves the codec at
   about 475 lines, doing reads and writes only. No behaviour change, and the
   existing `style_format.rs` refusal tests cover it. Do not ship the reason
   instead of the split: PR 2 to 4 add no keys, but the stop list helpers will
   grow when PR 4 needs reads in rank order.

## Non-blocking (fix in this PR if cheap, otherwise in PR 2)

2. **Merge renumbering: reduce the touchpoints.** If another bump
   (`ellipse-arcs-and-shaping`) merges first, these must all change today:
   `CURRENT_FORMAT_VERSION` and its doc paragraph (`document.rs`);
   `STYLES_FORMAT_VERSION = 7` and the literal `> 6` in `tests/style_format.rs`
   (`a_version_six_reader_would_refuse_this_build_so_the_bump_is_real`); the
   fixture name and bytes of `styles_v7.curvyo`; the pin
   `the_format_version_is_seven_until_the_merge_renumbers_it` in
   `curvyo-editor-wasm/tests/acceptance_corner_radii_part2_tester.rs`; the
   regenerated `future_format_version.curvyo`; the dated notes in `adrs.md` and
   `plan.md`. Three of these do not need to move:
   - `> 6` → `>= STYLES_FORMAT_VERSION`, so the constant is the only literal
     in that file.
   - `future_format_version.curvyo`: write it with a far-future version (for
     example `u32::MAX` or 9999) in `container.rs`'s generator, instead of
     `CURRENT_FORMAT_VERSION + 1`, and regenerate once. This stops a binary
     merge conflict between every pair of bump PRs (both regenerate it today).
     `future_format_version_project_is_refused_as_too_new` only checks
     `FormatTooNew`, so nothing else changes.
   - The single literal pin belongs to the slice that currently owns the
     number. Move it from the corner-radii tester file into
     `tests/style_format.rs` as `assert_eq!(CURRENT_FORMAT_VERSION as u64,
     STYLES_FORMAT_VERSION)`, give it a name that does not spell the number,
     and delete the corner-radii one. Its own version is already proven by
     its genuine v6 fixtures.
   Literal use otherwise is right: every other pin uses `CURRENT_FORMAT_VERSION`,
   and the older fixtures (`rotation_v5`, `legacy_corner_radius_v5`,
   `corner_radii_per_corner`, `paths_v2`, `primitives_v3`) stay as genuine old
   containers and are proven to open at the defaults with no write.

3. **`curvyo-document-core/src/styles.rs` `add_stop`: a second creation path
   for the `fill_stops` container** (ADR 0009 §3 convergence). `add_stop` calls
   `ensure_stops_list`, so on an object with no list it creates the container
   and a 1-stop list. A peer that runs `set_fill_mode` on the same object at
   the same time creates a different container under the same key, and Loro
   keeps only one of them. `adrs.md` accepts that race for the seed, but only
   for the seed. Fix: in `add_stop`, use
   `stops_list(&meta).ok_or(StyleEditError::NoSuchStop)?`, or add a
   `NoGradient` variant, so that only `set_fill_mode` (and Split's
   `write_style` on a fresh node) ever creates the container. The editor never
   adds a stop to an object that has no stops, so no criterion changes.

4. **`style_codec.rs`: unwraps under a shared invariant comment** (§5: "No
   `unwrap` ... unless preceded by an `// invariant: ...` comment"). Lines 233
   to 254 put one comment above four functions ("invariant for every insert
   below"). Fix: one `fn insert(meta, key, value: impl Into<LoroValue>)`
   helper with its own `// invariant:` line, called by `write_bool`,
   `write_number`, `write_text`, `write_color` and `write_dash`. That removes
   four `#[allow]`s.

5. **`style_codec.rs`: two pass-through aliases.** `read_stroke_width` only
   calls `read_width`, and `read_stop_map` only calls `read_stop`. Make the
   originals `pub(crate)` and delete the aliases (§5, no dead indirection).

6. **`specification.md` status line.** `Status: In progress` was set by the
   implementer. The status is the PO's field (`specs/README.md`). Harmless,
   but the lead should confirm it with the PO and not let it become a habit.

## Checked and fine

- **Document model and registers.** One LWW register per control. On/off is a
  flag separate from the values it gates. Dashes are stored as width ratios.
  Stops are a movable list of maps with a caller-minted `StopId` that is unique
  per object list (readiness check §3). "Unchanged value writes nothing" is
  implemented per object and per stop (`write_changes`, `edit_stops`), and the
  in-crate commit-count tests pin it. The exact (no tolerance) compare in
  `write_changes` is what register-granularity rule 5 requires, not a §5
  geometric comparison.
- **Format v7 on top of main's 6.** The doc paragraph is complete. Absent
  defaults are frozen and equal the old rendering. Creation still writes only
  `stroke_width` and `stroke` (`a_fresh_object_carries_only_the_width_and_the_colour`).
  `document.json` has one `style` object per object, with stop ids as hex.
  The golden `styles_v7.curvyo` holds all 12 keys across its four objects,
  plus a stop list with coincident stops whose creation order is pinned. Round
  trip and "opening writes nothing" (version vectors equal) are tested.
- **Future-version rejection.** This is unchanged logic (`container.rs`), and
  the regenerated fixture is refused. See 2 for the regeneration cost.
- **Legacy reads.** Strict validation now also covers `stroke_width` and
  `stroke` in v2 to v6 files, which used to degrade quietly. That is safe: no
  older writer could store an invalid value. Creation writes 0.25 and black as
  i64. Resize floors at 0.01 mm via `f64::max`, which is NaN-safe, and
  `stroke_or_radius_factor` clamps to non-negative values before `sqrt`. All
  five old fixtures open.
- **Open-file validation** matches every refusal case in `adrs.md`, accepts
  0, 1, 17 and 40 stops, and still tolerates unknown keys.
- **Units as types.** `Opacity` and `StopPosition` are validated newtypes with
  `try_from` serde, and `Length` is used for the width. Dash entries are
  dimensionless ratios, as `adrs.md` allows.
- **Crate boundaries.** No new crate, no `Cargo.toml` or `Cargo.lock` change,
  no new dependency. `document-core` builds for wasm32. No trait, no generic.
  `GradientStop::default_pair` in `document-core` is a pure function and is
  recorded in `adrs.md`. It is editor policy, not format, so it may change
  without a bump.
- **Public API.** It adds `Style`, `Stroke`, `Fill`, `GradientStop`, four
  newtypes, three enums, `StyleEdit`, `FillMode`, `FillModeTarget`,
  `StopEdit`, `StopChange`, two error enums and two limits. Each has a user in
  PR 3 and PR 4. Commands take resolved data and caller-minted ids, and
  resolve every id before the first write.
- **Resize commands.** The signatures are unchanged. They now refuse a width
  that is not finite or not above zero, before any write (`check_stroke_width`
  is called before `require_shape`). The off-switch regression tests are
  unchanged.
- **Churn in `ui-core`, `render-core` and `editor-wasm`.** It is mechanical
  and required by readiness check §2.4 and §2.5: the field rename and
  `PrimitiveSnapshot` losing `Copy`. `select_tool.rs` changes in tests only.
  `transform_drag`, `transform_commit` and `transform_primitive` are the
  re-pointing that §3 names. `render-core` reads `style.stroke.width` and
  `.color` only, so AC 3 holds. No behaviour change.
- **Split, Join, copy, "Object to path".** These follow readiness check §3:
  `create_path_uncommitted` takes `&Style` and writes the stops verbatim.
  Each operation has a test, and `no_style_key_is_a_primitive_key` guards
  conversion.
- **Other modules.** `style_model.rs` is 400 lines without tests and
  `styles.rs` 373, each with a one-sentence responsibility.
- **`docs/technical-debt.md`.** The measurement is added to the existing
  canvas-performance item and is below the 25 ms line, so the cache correctly
  stays deferred to PR 2's re-measure.
