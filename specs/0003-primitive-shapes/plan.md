# Plan for "Primitive shapes: rectangle, ellipse and polygon/star tools, and 'object to path'"

## Affected crates/modules

- `curvyo-document-core`: `units.rs` (+`Angle`); new `primitive_model.rs`
  (Shape data, validated newtypes, snapshots), `primitive_outline.rs`
  (closed-form outline math), `shape_codec.rs` (Loro value shapes/keys +
  open-file validation), `shapes.rs` (Document command methods).
  `path_codec.rs` (visibility bumps + shape-aware tree validation),
  `paths.rs` (`path()` returns `None` for a primitive node), `document.rs`
  (rename `PATHS_TREE`→`OBJECTS_TREE`, `format_version`→3, `document.json`
  `paths`→`objects`), `lib.rs` (new public exports).
- `curvyo-ui-core`: new `primitive_selection.rs` (shared selection across
  the three shape tools), `shape_hit_test.rs`, `handle_layout.rs`,
  `shape_tools.rs` (`RectangleTool`, `EllipseTool`, `PolygonStarTool`),
  `lib.rs` exports.
- `curvyo-render-core`: rename `primitives.rs`→`glyphs.rs`; new
  `shape_preview.rs` (primitive stroke + bbox + handle decorations,
  pen_preview.rs's pattern); `lib.rs` wiring.
- `curvyo-editor-wasm`: `session.rs` grows the three shape tools +
  object-to-path + decoration input; `wasm_api.rs` thin pass-through.
- `frontend/`: `ToolRail.tsx` (+3 buttons), new `ShapeToolbar.tsx`
  (point-count/ratio/mode), `Canvas.tsx` (cursor/keyboard wiring),
  `useEditorSession.ts` (+shape-tool state/actions), `editorSession.ts`
  (type re-exports).
- `curvyo-document-core/tests/fixtures/`: new `primitives_v3.curvyo` golden
  fixture + reuse of the existing genuine `paths_v2.curvyo` to prove the
  empty migration.

No new crate, no new external dependency (per `adrs.md`).

## Tasks

- [x] 1. `units.rs`: add `Angle` newtype (radians) + tests.
- [x] 2. `primitive_model.rs`: `RectBounds` (+`from_corners`), `EllipseFrame`,
      `StarFrame`, `PointCount` (3..=1024), `InnerRatio` (0,1 exclusive),
      `ShapeParamError`, `Shape` enum, `PrimitiveSnapshot`, `ObjectSnapshot`.
      (Fulfils AC 10's newtype validation.)
- [x] 3. `primitive_outline.rs`: `KAPPA`, `effective_corner_radius`,
      `rect_outline` (4 or 8 nodes), `ellipse_outline` (4 smooth nodes),
      `polygon_outline` (N corner nodes), `star_outline` (2N corner nodes),
      `outline_of(&Shape)`. Unit tests pin node counts/kinds/tolerances.
      (Fulfils AC 4, 5, 6, 18, 19, 20.)
- [x] 4. `shape_codec.rs`: keys, read/write helpers per shape, shape-tag
      read, `validate_primitive_node` (unknown shape / missing or
      mistyped field / non-finite / negative / point-count range /
      inner-ratio range → reject). Bump needed `path_codec.rs` helpers to
      `pub(crate)` for reuse. (Fulfils AC 10's file-open refusal.)
- [x] 5. `path_codec.rs`: `validate_path_tree` dispatches each root to
      path validation (shape absent) or `shape_codec::validate_primitive_node`
      (shape present); unknown extra keys on a path node remain tolerated.
- [x] 6. `shapes.rs`: `Document::create_rect/create_ellipse/create_polygon/
      create_star`, `set_rect_bounds/set_corner_radius/set_ellipse_frame/
      set_star_frame/set_point_count/set_inner_ratio`, `convert_to_paths`,
      `object(id)`, `primitive(id)`, `ShapeEditError`. Each setter/creator
      is one commit; `convert_to_paths` rewrites every named node in place
      in one commit, keeping its id, touching no style key. (Fulfils AC 1,
      2, 3, 6, 7, 8, 9, 11, 12, 13, 14, 15, 17, 21, 22.)
- [x] 7. `paths.rs`: `path(id)` returns `None` once a `shape` tag is
      present. `document.rs`: rename tree constant, bump
      `CURRENT_FORMAT_VERSION` to 3, `DocumentJsonView`'s `paths`→`objects`
      (tagged by shape), `from_loro_snapshot` unchanged call site (renamed
      constant only).
- [x] 8. `lib.rs`: export the new public types/functions.
- [x] 9. `curvyo-ui-core`: `primitive_selection.rs` (`PrimitiveSelection`,
      multi-id, same shape as `NodeSelection` minus the per-path scoping).
- [x] 10. `shape_hit_test.rs`: hit-test a point against a primitive's own
      outline (reusing `geometry-core::nearest_point_on_segment` per
      outline edge/curve) for selection, and against its handle positions
      for drags.
- [x] 11. `handle_layout.rs`: pure functions, `PrimitiveSnapshot` (+
      selection) → the handles to show and where (8 resize handles for
      rect/ellipse, 1 draggable + 3 display-only corner-radius handles for
      a selected rectangle, 4 resize + (star-only) 1 inner-ratio handle for
      polygon/star).
- [x] 12. `shape_tools.rs`: `RectangleTool` (AC 1-6), `EllipseTool` (AC
      7-9), `PolygonStarTool` (AC 10-15, holds persistent point-count/
      ratio/mode state, "not reset between shapes"). Each: `pointer_down`/
      `pointer_up` for create-drag and for select+resize+param-handle
      drags, one commit per gesture, a zero-movement create writes
      nothing (AC 1, 7, 11, 12).
- [x] 13. `curvyo-ui-core::lib.rs` exports.
- [x] 14. `curvyo-render-core`: rename `primitives.rs`→`glyphs.rs`
      (update `lib.rs`/`decorations.rs`/`pen_preview.rs`/`stroke.rs`
      imports), confirming the module now only means UI glyphs.
- [x] 15. `shape_preview.rs`: primitive stroke (outline → existing lyon
      stroke path, AC 16's identical 0.25mm/black/no-fill), bounding-box
      selection outline, shape-handle glyphs (8px hollow square /
      filled-on-drag), dashed corner-radius guide. Reuses
      `docs/design-system.md`'s tokens (added to `theme.rs`).
- [x] 16. `curvyo-editor-wasm::session.rs`: `Tool::Rectangle/Ellipse/
      PolygonStar`; dispatch pointer events to the active shape tool;
      `convert_selected_to_paths` (AC 17, 22); decoration input for shape
      handles; polygon/star tool-options getters/setters.
- [x] 17. `wasm_api.rs`: thin pass-through bindings for the above.
- [x] 18. Frontend: `ToolRail.tsx` (+Rectangle/Ellipse/Polygon-star
      buttons, appended, not reordering Pen/Node), `R`/`E`/`*` shortcuts
      in `useEditorSession.ts`'s `onKeyDown`, a `ShapeToolbar.tsx`
      (mode/point-count/ratio, node-toolbar's pattern) incl. an
      "Object to path" action, `Canvas.tsx` wiring unchanged pattern.
- [x] 19. `curvyo-document-core` golden fixtures: regenerate
      `tests/fixtures/*.curvyo` including a genuine `format_version=3` file
      with one of each primitive kind (`primitives_v3.curvyo`), and reuse the
      existing genuine `paths_v2.curvyo` (slice 2) unchanged to prove the
      empty-migration path still opens under the new `CURRENT_FORMAT_
      VERSION = 3` build. Golden tests in `container_fixtures.rs`.
- [x] 20. Run the full `CLAUDE.md` §7 gate; fix findings.

## Validation

- Unit tests in `curvyo-document-core` pin the exact node
  count/kind/segment-type for AC 18-20, the kappa-based 0.1%-of-larger-
  radius tolerance for AC 19, the half-shorter-side clamp for AC 5
  (including the "clamped on read, not on write" round-trip), and the
  3..=1024 / 0<R<1 newtype boundaries for AC 10/12.
- Unit tests in `curvyo-ui-core` exercise each tool's create-drag
  (zero-movement writes nothing), handle-drag arithmetic (resize keeps
  radius/ratio/count; radius/ratio/count-only edits keep the frame), and
  multi-select "object to path" (AC 22) converting each primitive
  independently in one call.
- Golden-file round trip: `primitives_v3.curvyo` (pack → unpack → re-pack
  byte-for-byte document.json fields) plus the existing genuine
  `paths_v2.curvyo` opening unchanged under the new `CURRENT_FORMAT_VERSION`.
- `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets`,
  `cargo fmt --all --check`, wasm32 build of every `*-core` crate,
  `cargo deny check`, `cargo doc` with `-D warnings` — `CLAUDE.md` §7.
