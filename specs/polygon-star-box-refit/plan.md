# Plan for Polygon and star: the selection box turns with the shape

Spec: [specification.md](specification.md), ADR notes: [adrs.md](adrs.md)
(both arrive with docs PR #48). Approach (a) of `adrs.md` decision 1: the box
direction is `ObjectSnapshot::orientation()`, derived in `oriented_bounds`.

## Affected crates/modules

- `vecmanf-ui-core`: `oriented_box.rs` (the one expression),
  `param_handles.rs::star_inner_vertex` and `param_edit.rs::value_from_pointer`
  (drop `frame.angle`, in box-local coordinates the first outer vertex is at 0),
  `transform_primitive.rs::resize_primitive` (pin helper gets `start_box.angle`),
  new `tests/polygon_star_box_refit.rs`.
- `vecmanf-editor-wasm`: `session/select_view.rs::cursor_hint` (box angle, not
  the `rotation` register), new `tests/polygon_star_box_refit.rs`.
- Existing tests whose expectation names a box direction or handle of a polygon
  or star with a non-zero `StarFrame.angle` (list in the PR).
- Not touched: `vecmanf-document-core`, `vecmanf-render-core`, `vecmanf-app`,
  `frontend/`, any `Cargo.toml`. No stored field, `format_version` stays 5.

## Tasks

- [x] 1. Golden numbers from `main` for the old inner-vertex positions and the
  rectangle, ellipse and path boxes, pinned in tests before the change
  (fulfils AC 8, 15).
- [x] 2. Failing ui-core tests: box table over (frame angle, rotation),
  worked corners, invariant proptest (all outline vertices inside the box,
  first outer vertex at local angle 0, star handle on the real inner vertex,
  rotate by delta turns the box by delta) (fulfils AC 1, 2, 4, 8, 13).
- [x] 3. `oriented_bounds` sets `angle: object.orientation()`; fix
  `star_inner_vertex`, `value_from_pointer`, `resize_primitive` pin argument
  (fulfils AC 1, 7, 8, 9).
- [x] 4. Failing session tests, then `cursor_hint` uses the box angle:
  create-drag box direction, typed rotation table, drag rotate with and
  without Shift and Ctrl (preview equals commit), resize example, typed
  radius, cursor strings, hit rule, no skew handle, no stored-field write,
  old fixtures (fulfils AC 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15).
- [x] 5. Update the existing tests whose expectation moved with the box
  direction; list each in the PR (fulfils AC 1, 2).
- [x] 6. Docs: `docs/technical-debt.md` and `specs/edit-interaction-polish`
  notes are in PR #48; nothing else changes here. Module doc of
  `oriented_box.rs` updated.
- [x] 7. Full gate (CLAUDE.md §7 plus everything `.github/workflows/ci.yml`
  runs), check the UI in the Browser pane, draft PR, CI green on the head sha.

## Validation

- Property test over centre, radius, point count, ratio, frame angle and
  rotation for the box invariants; golden numbers from `main` for the star
  handle position and for rectangle, ellipse and path boxes.
- Session-level tests through `Session`'s public API for every gesture of the
  spec; the stored registers are compared before and after each gesture.
- Manual check in the browser: polygon at an angle, R 0 Enter, handles,
  pivots, no skew handle, star inner handle at several angles.
