# Plan: object-transform (slice 5)

Branch: `story/object-transform`, worktree
`/home/marc/workbench/vecmanf-claude-object-transform`, branched from
`origin/main` at `a7e82f5` (`path-merge-split-and-node-types`, #26).

`main`'s `CURRENT_FORMAT_VERSION` is **4** at branch time, confirmed by
reading `vecmanf-document-core/src/document.rs` on `origin/main` before
branching. This slice therefore takes **5**, matching the architect's own
2026-10-05 resolution note in `adrs.md` (not the plain "4" the dated
decision above it says) — no discrepancy to report.

## Scope

Every acceptance criterion is in scope, including the on-canvas readout,
the cursors and the glyph shapes (a first pass of this plan deferred
those; the lead rejected that — they are criteria 1, 14, 18, 22 and the
UX notes, not follow-ups).

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

- [x] 1. `Angle::normalized`, `Point::rotated_around`, `Vec2::rotated` (infrastructure)
- [x] 2. `rotation` on snapshots, codec, `Damaged` on non-finite/mistyped (19, 24)
- [x] 3. `CURRENT_FORMAT_VERSION = 5`, `document.json` carries `rotation`; golden fixture `rotation_v5.vmf`, `future_format_version.vmf` regenerated (19, 24)
- [x] 4. `shape_center`, `rotate_shape`, `ObjectSnapshot::rotated` (15-18, 20, 21)
- [x] 5. `PathSnapshot::rotated`/`scaled` (12, 20)
- [x] 6. `outline_of_rotated`; **used by render-core's primitive stroke and live preview too** (an earlier pass of this slice forgot render-core) (17, 21, 25)
- [x] 7. `Document::rotate_object` (15-18)
- [x] 8. `resize_rect`/`resize_ellipse`/`resize_star_frame`/`resize_path` (4-13)
- [x] 9. Split carries `rotation` (architect note)
- [x] 10. `conversion.rs`/`hit_test_object.rs` use the rotated outline (17, 21, 25)
- [x] 11. `OrientedBox` (+ `document_corners`) (1, 18)
- [x] 12. `transform_handle_layout`: handles, hit test, resize/rotate arithmetic, stroke/radius factor, resize-cursor angle (4-17, cursors)
- [x] 13. `SelectTool` resize/rotate drags; a rotated primitive's resize pins the anchor in document space (1-3, 4-18, 23)
- [x] 14. render-core: oriented selection/hover outline (select tool and shape tools), squircle resize handles, circular-arrow rotate handle with idle/hover/dragging looks, pivot marker (1, 18, UX notes)
- [x] 15. Live numeric readout (size / angle) through the existing `LiveReadout` channel (14, 22)
- [x] 16. Cursors: `Session::cursor_hint`, `wasm_api::cursor_hint`, `frontend/src/lib/cursors.ts` (rotated resize cursor with built-in fallback; static rotate cursor) (UX notes)
- [x] 17. Shift/Ctrl threaded through `pointer_hover`/`pointer_up`, re-run on modifier key press/release (5, 7, 16, 17)
- [x] 18. Shape tools' own handles follow rotation: `handles_for` rotated, drag deltas mapped into local axes, live preview rotated (25)
- [x] 19. Frontend builds (`npm install`, `npm run build`: `tsc -b` + `vite build`), `vecmanf-app` builds and is in the gate

## Known limitations (not hidden)

- The shape tools' *own* resize of a rotated primitive keeps the opposite
  edge fixed in the local frame, so on screen the opposite corner drifts
  (the Select tool's resize pins it; the shape tools' arithmetic is
  unchanged per `adrs.md`). Criterion 25 asks only for handle position and
  axes, which are covered.
- The glyphs (squircle, circular arrow) and the cursor images are simple
  first versions for the ux-engineer to review; no criterion tests their
  exact shape.
- The readout offset is now 12 px for every tool (it was 8 px for the shape
  tools' create readout), per this slice's UX notes.

## Validation

- `cargo test -p vecmanf-document-core -p vecmanf-ui-core` for every task
  above, test-first.
- Full gate (`CLAUDE.md` §7) before reporting done.
