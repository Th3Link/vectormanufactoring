# Plan: object-transform (slice 5)

Branch: `story/object-transform`, worktree
`/home/marc/workbench/vecmanf-claude-object-transform`, branched from
`origin/main` at `a7e82f5` (`path-merge-split-and-node-types`, #26).

`main`'s `CURRENT_FORMAT_VERSION` is **4** at branch time, confirmed by
reading `vecmanf-document-core/src/document.rs` on `origin/main` before
branching. This slice therefore takes **5**, matching the architect's own
2026-10-05 resolution note in `adrs.md` (not the plain "4" the dated
decision above it says) — no discrepancy to report.

## Scope decision (effort-bounded)

This spec's 25 acceptance criteria span document-model data (rotation),
pure geometry/interaction logic (ui-core), rendering decorations
(render-core) and device-specific chrome (cursors, on-canvas readouts —
editor-wasm/frontend). Per `CLAUDE.md` §4 "core logic test-first, UI wiring
thin": document-core and ui-core get full test coverage against the
acceptance criteria below. render-core gets the decoration data plumbed
through (quads + handle positions) at the same fidelity slice 4's own
decorations use. Exact cursor glyphs/CSS and the live on-canvas numeric
readout's pixel placement are frontend/editor-wasm concerns outside any
crate with unit tests today (editor-wasm has none) — flagged in the final
report as follow-up for ux-engineer/frontend wiring, not silently dropped.

## Affected crates/modules

- `vecmanf-document-core`: `units.rs` (Angle::normalized, Point/Vec2
  rotation arithmetic), `path_model.rs` (`PathSnapshot::rotation`, Join/
  Split-safe), `primitive_model.rs` (`PrimitiveSnapshot::rotation`,
  `ObjectSnapshot::rotated`, `shape_center`, `rotate_shape`),
  `path_codec.rs`/`shape_codec.rs` (rotation key, write_stroke_width,
  validation), `document.rs` (`CURRENT_FORMAT_VERSION = 5`, JSON export),
  `primitive_outline.rs` (`outline_of_rotated`), `objects.rs`
  (`Document::rotate_object`), `shapes.rs` (`resize_rect`/`resize_ellipse`/
  `resize_star_frame`), `paths.rs` (`resize_path`), `path_topology.rs`
  (Split copies rotation to the new object).
- `vecmanf-ui-core`: new `oriented_box.rs` (OrientedBox type + local/
  document mapping), new `transform_handle_layout.rs` (8 resize + 1 rotate
  handle layout, hit test, stroke/radius factor arithmetic, Ctrl/Shift
  rules), `select_tool.rs` (resize/rotate drag states alongside the
  existing move drag), `hit_test_object.rs`/`conversion.rs`/
  `object_bounds.rs` updated for rotation-aware outlines.
- `vecmanf-render-core`: `select_decoration.rs` extended with transform
  handle quads (thin — reuses existing decoration conventions).

## Tasks (acceptance criteria in parens)

1. `Angle::normalized`, `Point::rotated_around`, `Vec2::rotated` in
   `units.rs`, tested. (infrastructure for 15-20)
2. `rotation: Angle` on `PrimitiveSnapshot` and `PathSnapshot`; codec
   read/write (absent = 0, normalized on write); open-file validation
   refuses non-finite `rotation` (`OpenError::Damaged`). (19, 24)
3. `CURRENT_FORMAT_VERSION = 5`; `document.json` gains `rotation` per
   object. (19, 24)
4. `shape_center`, `rotate_shape`, `ObjectSnapshot::rotated` (primitive:
   frame center + rotation register; path: delegates to
   `PathSnapshot::rotated`). (15-18, 20, 21, 25)
5. `PathSnapshot::rotated`/`scaled` (bake anchors + handles; `scaled` maps
   into the path's own local frame via its `rotation`, per axis, back
   out). (12, 20)
6. `outline_of_rotated` (rotated primitive outline; single place rotation
   applies for rendering/hit-test/object-to-path). (17, 21, 25)
7. `Document::rotate_object` (one commit, dispatches path/primitive).
   (15-18)
8. `Document::resize_rect`/`resize_ellipse`/`resize_star_frame`/
   `resize_path` (frame + corner_radius/stroke_width in one commit).
   (4-13)
9. `path_topology::split_open_path` carries the original's rotation to
   the new object. (20, architect note)
10. `conversion.rs`/`hit_test_object.rs` use `outline_of_rotated`;
    object-to-path keeps `rotation` (not stripped). (17, 21, 25)
11. `OrientedBox` (ui-core): local frame + angle, corner/edge-midpoint
    positions, document-space mapping — primitive via `shape_center`
    pivot, path via document-origin pivot + derotated anchor bounds.
    (1, 18)
12. `transform_handle_layout.rs`: 8 resize + rotate handle local
    positions (polygon/star: corners only, AC11); hit test; drag
    arithmetic for free/proportional/edge resize with opposite-corner or
    Shift-center pivot (4-7); stroke-width/corner-radius √(sx·sy) factor
    with zero-floor refusal (8, 9); 15° Ctrl snap for rotate (17);
    Shift bottom-edge-midpoint pivot for rotate (16); zero-size clamp
    (13). (4-17)
13. `select_tool.rs`: resize/rotate drag states gated to single-object
    selection (1, 2); press-release-no-move writes nothing (3); move
    unaffected (23); commits via the new Document methods.
14. Thin `render-core` decoration pass-through for the 8+1 handles
    (follow-up note for ux-engineer on exact glyphs/cursors).

## Validation

- `cargo test -p vecmanf-document-core -p vecmanf-ui-core` for every task
  above, test-first.
- Full gate (`CLAUDE.md` §7) before reporting done.
