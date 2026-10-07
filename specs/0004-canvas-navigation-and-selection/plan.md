# Plan for "Canvas navigation and a general Select tool"

## Affected crates/modules

- `curvyo-document-core`: `primitive_model.rs` (`ObjectSnapshot::translated`,
  `translate_shape` helper), `primitive_outline.rs` (`shape_frame_bounds`,
  moved from `render-core`), new `objects.rs` (`Document::translate_objects`,
  `Document::delete_objects`, `ObjectEditError`).
- `curvyo-geometry-core`: `segment.rs` gains `segment_bounds`.
- `curvyo-ui-core`: new `viewport.rs` (`Zoom`, `Viewport`, pan gesture,
  zoom-about-a-point, resize), new `select_tool.rs` (`SelectTool`,
  double-click outcome), new `hit_test_object.rs` (`hit_test_object` + shared
  anchor-run helper), new `object_bounds.rs` (`object_bounds`),
  `primitive_selection.rs` renamed to `object_selection.rs`
  (`ObjectSelection`), `shape_hit_test.rs`'s `hit_test_primitive` now
  delegates to `hit_test_object`, every shape-tool file's `PrimitiveSelection`
  references renamed.
- `curvyo-render-core`: `theme.rs` (screen-space display tolerance tokens,
  min stroke width), `stroke.rs`/`pen_preview.rs`/`shape_preview.rs`
  (view-dependent display tolerance + 1px-minimum display stroke width),
  `decorations.rs` gains the Select-tool bounding-box decoration input and
  geometry, `shape_preview.rs`'s private `bounding_box` replaced by the
  moved `curvyo_document_core::shape_frame_bounds`.
- `curvyo-editor-wasm`: `session/mod.rs` (`Tool::Select`, `Viewport` field
  replacing the bare `ViewTransform`, `screen_to_document`, wheel/pan/resize
  plumbing, `double_click`), new `session/select.rs` (Select-tool glue,
  `tool_for`/`tool_for_shape`), `session/shapes.rs` (`shape_matches_active_tool`
  derived from `tool_for_shape`), `gpu.rs` (f64 origin subtraction before the
  f32 vertex cast), `wasm_api.rs` (new screen-pixel pointer/wheel/pan/resize/
  zoom-percent methods, `set_view` removed, `"select"` tool string).
- `frontend/`: `ToolRail.tsx` (Select button first, floating panel CSS),
  `useEditorSession.ts` (non-passive wheel listener, Space tracking, drops
  `CSS_PX_PER_MM`/`documentPoint`, double-click now calls one
  `double_click(x,y)`), `StatusBar.tsx` (zoom center segment), `Canvas.tsx`
  (pan cursor classes).

## Tasks

- [x] 1. `curvyo-geometry-core::segment_bounds` — exact cubic-segment bounds
      via `kurbo`'s `ParamCurveExtrema`. Test-first. (fulfils AC 14)
- [x] 2. `curvyo-document-core`: `shape_frame_bounds(&Shape)`, moved from
      `render-core`'s private `bounding_box`; `translate_shape(Shape, Vec2)`;
      `ObjectSnapshot::translated(Vec2)`. Test-first. (fulfils AC 14, 18, 20)
- [x] 3. `curvyo-document-core::objects`: `Document::translate_objects` and
      `Document::delete_objects`, one commit each, resolve-before-write,
      reusing `translate_shape` for primitives. Test-first — including the
      one-commit-per-batch check the existing shape methods already pin.
      (fulfils AC 18, 19, 20, 21)
- [x] 4. `curvyo-ui-core::viewport`: `Zoom` (clamped `0.02..=80.0`),
      `PX_PER_MM_AT_100`, `Viewport` (pan by screen delta, zoom-about-a-point,
      drag-pan gesture, resize-keeps-center). Test-first, including the
      clamp-boundary cursor-fixed-point test called out explicitly by
      `adrs.md`. (fulfils AC 1-11)
- [x] 5. `curvyo-ui-core`: rename `PrimitiveSelection` → `ObjectSelection`
      (file + all call sites across `rectangle_tool.rs`, `ellipse_tool.rs`,
      `poly_star_tool.rs`, `shape_tool_common.rs`, tests). (fulfils AC 16, 17)
- [x] 6. `curvyo-ui-core::hit_test_object` — shared anchor-run distance
      helper + public `hit_test_object(&[ObjectSnapshot], Point, Tolerance)`;
      `shape_hit_test::hit_test_primitive` delegates to it. Test-first,
      including the z-order tie-break. (fulfils AC 14)
- [x] 7. `curvyo-ui-core::object_bounds` — primitive via
      `shape_frame_bounds`, path via the union of `segment_bounds` over every
      segment. Test-first. (fulfils AC 14)
- [x] 8. `curvyo-ui-core::select_tool` — `SelectTool` (click/shift-toggle via
      `hit_test_object`, single-object drag-to-move tracked as a screen-space
      offset, Delete), plus a free `double_click` outcome function. Test-first.
      (fulfils AC 14, 15, 16, 17, 18, 19, 20, 21, 22, 23)
- [x] 9. `curvyo-render-core`: screen-space `DISPLAY_TOLERANCE_MM` (`0.25px /
      scale`) threaded through `stroke::path_stroke`/`segment_stroke` call
      sites; 1px-minimum on-screen stroke width for committed strokes (flag 2,
      default (a)); new `SelectionBoxInput`/decoration geometry for the
      Select tool's plain bounding box (selected/hovered, per object, no
      handles). Test-first. (fulfils AC 7, 14, 17 UX notes)
- [x] 10. `curvyo-editor-wasm::gpu`: subtract the view origin in `f64` before
      the `f32` vertex cast; `ScreenTransform`'s offset no longer re-subtracts
      origin. (fulfils AC 7's high-zoom-far-from-origin precision note)
- [x] 11. `curvyo-editor-wasm::session`: `Tool::Select` (new launch default
      for `new`/`open`), `Viewport` field, `screen_to_document`, `wheel`,
      `begin_pan`/`pan_to`/`end_pan`, `resize_viewport`, `zoom_percent`,
      `double_click` dispatch via `session/select.rs`'s `tool_for`. Resize
      keeps center. Test-first for every dispatch branch. (fulfils AC 1-13,
      22, 23, 24)
- [x] 12. `curvyo-editor-wasm::wasm_api`: screen-pixel `pointer_down`/
      `pointer_hover`/`pointer_up`, `wheel`, `begin_pan`/`pan_to`/`end_pan`,
      `double_click`, `zoom_percent`; `resize` also updates the viewport's
      canvas size; `set_view` removed; `"select"` added to `tool_from_str`/
      `tool`. (fulfils AC 1-13, 22, 23, 24)
- [x] 13. Frontend wiring: `ToolRail.tsx` Select button first + `S` shortcut;
      non-passive wheel listener with deltaMode normalization; Space-held
      tracking swapping pointer-down/move/up to the pan entry points on
      middle-button or Space+primary; double-click now calls one
      `double_click(x,y)`; `StatusBar.tsx` zoom center segment; pan cursor
      classes in `Canvas.tsx`; drop `CSS_PX_PER_MM`/`documentPoint`/`set_view`.
      (fulfils AC 1-13, 22, 23, 24)
- [x] 14. Full quality gate (`CLAUDE.md` §7).

## Validation

- Unit tests at every layer above (pure logic, no UI needed for most of it).
- A dedicated unit test for the zoom clamp-boundary case: zooming toward a
  cursor point when the target scale would exceed `8000%`/`2%` still leaves
  that exact document point under the cursor after clamping (recompute origin
  from the *clamped* scale, not the pre-clamp target).
- `cargo build -q --target wasm32-unknown-unknown` for every `*-core` crate
  confirms the wasm boundary still holds (no new non-wasm dependency, no
  `document-core`/`geometry-core`/`ui-core`/`render-core` type crossing into
  platform code).
- No golden-file/format changes: `format_version` stays `3`, confirmed by the
  existing round-trip tests still passing unmodified.
- Manual smoke check in the running app (via `npm run tauri dev` or the
  browser preview) for the parts that are host-input-shaped and not fully
  unit-testable: wheel pan/zoom feel, middle-drag/Space-drag pan, resize
  keeping the canvas live (no blank frame), Select-tool click/drag/delete/
  double-click handoff.
