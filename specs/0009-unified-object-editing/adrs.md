# ADRs for "Unified object editing: one Select tool for every object and its own handles"

This feature moves the primitives' own editing (corner radius, star inner
radius, point count, ratio, "Remove rounding", "Object to path") into the
Select tool and makes the three shape tools creation-only. Every value it
writes goes to a register that `primitive-shapes` and `object-transform`
already write, through `Document` commands that already exist
(`set_corner_radius`, `set_inner_ratio`, `set_point_count`, `resize_rect`,
`resize_ellipse`, `resize_star_frame`, `rotate_object`, `translate_objects`,
`convert_to_paths`). **No document-model change, no `format_version` change
(it stays at `main`'s 5), no new crate, no new external dependency, no new
`curvyo-document-core` or `curvyo-geometry-core` function, no ADR
amendment.** Criteria 24 and 38 are confirmed buildable on that basis.

Reference state: `main` after PR #35. Everything below was checked against the
code of that state. Nothing in the specification is unbuildable as a whole;
seven criteria are unbuildable or contradictory as written and carry a default
under "Flagged to the lead".

## Depends on

- [ADR 0001 §1, §4](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  handle layout, the one hit rule, the drag and entry arithmetic, the bar's
  state and the live-edit resolution live in `curvyo-ui-core`; the draw list
  in `curvyo-render-core`; `Session` and the frontend forward events and
  render.
- [ADR 0001 §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  new wasm calls carry scalars and strings only.
- [ADR 0002 §3](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  radius and ratio stay `Length` and `InnerRatio` end to end, never a bare
  `f64` at an API edge; every comparison below names its tolerance.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one drag, one confirmed entry, one slider interaction is one commit; no
  movement, no change, or a drag back to its start writes nothing (criterion
  12; decision 5 closes a hole in today's move commit).
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md):
  the preview, the two switches, the bar's pending slider value and the open
  entry are ephemeral; a resize that leaves the radius alone must not rewrite
  its register (decision 4).
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  no new key and no new meaning of a key.
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md): every
  piece fits an existing crate over an existing edge.
- [`specs/0003-primitive-shapes/adrs.md`](../0003-primitive-shapes/adrs.md):
  the stored radius is raw and clamped only on evaluation
  (`effective_corner_radius`); "Object to path" keeps the `NodeId`.
- [`specs/0005-object-transform/adrs.md`](../0005-object-transform/adrs.md) and
  [`specs/0008-object-transform-refinements/adrs.md`](../0008-object-transform-refinements/adrs.md):
  one resolving function per gesture shared by preview, release and typed
  entry; the nearest-centre hit rule; `StrokeScaling` captured at the press or
  when the entry opens; the 3 px dead zone (`DragOrigin`); frozen handle set
  during a drag. This feature adds a handle family and a second switch to that
  machinery; it adds no second drag or hit rule.
- [`specs/0011-shape-creation-from-center/adrs.md`](../0011-shape-creation-from-center/adrs.md):
  its `create_drag_box` in `shape_tool_common.rs` is the only create-drag
  computation after this feature; this feature does not duplicate it.

## Feature-local decisions

- **2026-10-06: the crate boundary.**

  | Concern | `document-core` | `ui-core` | `render-core` | `editor-wasm` / `frontend/` |
  |---|---|---|---|---|
  | Parameter handle registry, layout, hit radius, tiers (1-8) | none | new `param_handles.rs`; `EditHandle::Param`; fields added to `TransformHandleTolerances` | one glyph kind `TransformGlyphKind::Parameter` | forwards, cursor and hint strings |
  | Drag, typed entry and bar edit of a parameter (2, 3, 9, 18-21) | none (existing commands) | new `param_edit.rs` (value, resolve, commit), `param_entry.rs` | none | binding; DOM chip kinds |
  | Blue-new / black-old (10-15) | none | `LiveEdit` in `select_tool/preview.rs` | new `live_preview.rs`: `build_live_edit_preview(&[ObjectSnapshot])` | `Session::draw_list` composition |
  | "Scale corner radius" (23) | none | `ScaleModes` replaces the lone `StrokeScaling` on `SelectTool` | none | `Session` accessor pair; one switch component |
  | Select bar state (21, 22) | none | new `select_bar.rs`: pure `select_bar_state` | none | `Session` and `wasm_select_bar.rs`; `SelectToolbar.tsx` |
  | Creation-only tools (25-30) | none | the three tool files shrink; editing helpers deleted | `ShapeDecorationInput` and shape-handle glyphs deleted | `Session` selection decoration for creation tools; bars |
  | Double-click (31-34) | none | `SelectDoubleClickOutcome` loses the primitive handoff | none | `tool_for` becomes path-only |

  No new crate: every piece has an owner crate whose responsibility sentence
  does not gain an "and". No trait is introduced; `ParamHandle` and
  `ParamValue` are enums with exhaustive `match`es, so a new parameter kind is
  a compile-driven edit in a small, known set of files.

- **2026-10-06: one handle registry, one hit rule.** Options: (A) a
  `ParamHandle` enum folded into the existing handle enum, laid out by one
  function per kind and hit-tested by the existing nearest-centre function;
  chosen. (B) Keep `handle_layout::ShapeHandle` / `hit_test_handle` as a second
  hit test and run both in `pointer_down`. Rejected: two hit tests can
  disagree about a tie, and criterion 5 asks for one.
  - `TransformHandle` becomes `EditHandle` (a pure rename in its own commit;
    the name would lie once a parameter variant exists) with the variants
    `Resize`, `Rotate`, `Skew`, `Move` unchanged and one new
    `Param(ParamHandle)`. `ParamHandle` is `CornerRadius(Corner)` with
    `Corner { Tl, Tr, Br, Bl }` (four values already in this spec, because the
    four handles are four distinct hit targets that all write one radius) and
    `InnerRadius`. `specs/0013-rectangle-corner-radii/` changes only what a
    `CornerRadius` drag writes; `specs/0021-ellipse-arcs-and-shaping/` adds
    `ArcStart`, `ArcEnd` and `Curve`. Nothing else in the registry changes.
  - **Layout.** `param_handles(snapshot, box, tolerances) -> Vec<(ParamHandle,
    Point)>`, one `match` on the primitive kind, in the primitive's local frame
    mapped through `OrientedBox::to_document`. Handles that are not drawn are
    not returned (criterion 6: a handle that is not drawn has no hit area), so
    `is_drawn_handle` needs no param arm and the hit function can never see an
    undrawn one. A param drag maps the pointer with `OrientedBox::to_local`
    (pivot and rotation only) and never reads the box extents, so a later arc
    whose tight box changes during the drag does not move the frame under it.
  - **Hit rule.** `hit_transform_handle` gains one arm: a `Param` handle's
    radius is `resize_radius` (the existing shrinking radius, one third of the
    shorter side, between 4 and 16 px; **superseded: 12 px `param_hit`, see the
    second-pass note**), rank 0 so a param handle wins an exact
    tie (criterion 5), no inner-band rule. The `Move` handle stays a hover-only
    fallback. One function still serves press, hover, cursor, hint and
    double-click.
  - **Position at radius 0 and the zero position (flag 1). Superseded by the
    second-pass note (15 px inset, normalised map, drag gain).** The corner-radius
    handle sits on the diagonal at `d0 + r_eff` from its corner, `d0` a fixed
    screen inset (`TransformHandleTolerances::param_inset_mm`, 12 px proposed,
    ux sets it), slope 1 so the handle follows the pointer exactly. The drag
    stays delta-based, as `corner_radius_from_drag` is today: radius = start
    effective radius + the pointer's displacement projected on the corner's
    inward diagonal, clamped to `[0, half the shorter side]`. Criterion 2's
    "exactly 0 at or beyond the zero position" is the clamp at 0; the inset
    changes nothing in the arithmetic. The drag starts from the **effective**
    radius, as today, not the stored one.
  - **Tiers on small boxes. Superseded in its numbers by the second-pass
    note (`T` 72, clearance by construction); the tier order and the
    centre-yields rule stand.** The shorter side `s` in screen pixels decides:
    `s` below 24: corner resize and rotate handles; 24 to 48: plus edge resize;
    48 up to the parameter threshold `T`: plus the centre handle; `T` and
    above: plus the parameter handles. `T` is a tolerance field
    (`param_min_side_mm`), set by the `ux-engineer`. Two layout rules make the
    rest hold: (1) the centre glyph is hover-only and never hit-tested
    (refinements), so it is the one glyph that yields: it is not drawn when its
    edge is within 4 px of any drawn parameter glyph; (2) at `s = T` the four
    radius handles at the largest radius must clear each other and the resize
    glyphs by 4 px, which gives `T` of about 100 px at slope 1 and `d0` 12 px
    (criterion 7's 64 px cannot hold, flag 1). A property test over size,
    aspect, radius and rotation asserts the 4 px clearance and that the body
    stays reachable (a press at the box centre and at a point a quarter of the
    shorter side in from each corner is a move). **Corrected 2026-10-07
    (`rectangle-corner-radii` review): this reachability claim is false as
    stated; see the dated note at the end of this file.**
  - A star's inner-radius handle is laid out where `handle_layout` puts it
    today (the first inner vertex), in the shape's local frame. Its only
    possible collision is the centre glyph (rule 1). **Wrong, corrected in the
    second-pass note: at ratio 0.99 it also comes within 4.6 px of a corner
    resize glyph, and that case sets `T`.**

- **2026-10-06 (second pass, after the `ux-engineer` notes): the radius map,
  the tiers, the hit radius, the drawn rule and the clearance test. Confirms
  all five departures; supersedes the "Hit rule" radius sentence, "Position
  at radius 0", "Tiers on small boxes" and the star-handle bullet above, which
  stay for history.** Checked by hand against `specification.md` "UX notes" §1
  and the code (`transform_handle_layout.rs`, `handle_layout.rs`,
  `oriented_box.rs`). Nothing here changes a crate, a model, a command or the
  decision count of the registry.
  1. **Radius handle: CONFIRMED (15 px inset, far end `(s-14)/√2`, drag gain).**
     With `ρ = r_eff / (s/2)` (document side only: `ρ` does not depend on zoom),
     `L(s) = (s-14)/√2 - 15` and handle distance `p = 15 + ρ·L(s)` along the
     inward diagonal. Maths holds: the position is linear in `ρ`; `ρ = 1` puts
     the handle `(s-14)/2` from its corner on each axis, so two handles on the
     shorter side are exactly 14 px apart at every `s` and every aspect (longer
     sides only add distance; centre distance falls monotonically with `ρ`, so
     `ρ = 1` is the minimum), 4 px between two 10 px glyphs. At `ρ = 0` the knob
     is 4.3 px from the corner glyph's circumscribed circle (15 - 5.66 - 5).
     Edge and rotate glyphs are far (nearest knob to an edge glyph is 25 px at
     `s` 72). `L > 0` needs `s > 14 + 15√2 = 35.2 px`; `T = 72` guarantees it,
     and a unit test pins `L(T) > 0`. Gain `G = (s/2)/L`: 1.38, 1.09, 0.86, to
     0.71 at the large end, as stated. The inverse of the position map is a
     scalar, so my earlier "compressed map with an inverse in the drag" costs
     one multiplication, not a second rule; flag 1's default becomes this map.
     - **Delta-based drag stays.** `r = r_start_eff + G·projection`, clamped to
       `[0, half the shorter side]`, from the effective radius. The handle stays
       under the pointer exactly (the map is linear and `G` is its slope
       inverse), the press offset inside the hit radius is kept, and the zero
       position is the 15 px point. Gain applies to `CornerRadius` only; the
       star's inner-ratio arithmetic is unchanged (1 px per px).
     - **Frozen at the press.** `G` depends on the screen scale, so it is
       computed once at the press from the box's shorter side and the
       tolerances and stored in the drag (`ParamDrag { start, gain }`, with the
       `DragOrigin`). A wheel zoom during the drag then cannot change the
       mapping between frames, and release equals preview because `resolve`
       reads the same stored gain. Typed entry and the bar field never use the
       gain: they call `apply_param` with an absolute `Length`, which holds the
       clamps, so drag, entry and bar still agree on every value.
       `value_from_pointer` stays one pure function of `(start snapshot, local
       delta, gain)`; it returns the start for a non-finite input or `gain`.
     - **Tolerance fields** (`TransformHandleTolerances`, millimetres like the
       others): `param_inset_mm` 15 px, `param_pitch_mm` 14 px (minimum centre
       distance of two radius handles = knob diameter 10 + 4), `param_min_side_mm`
       72 px, `param_hit: Tolerance` 12 px. `L` and `G` are two small pure
       functions next to `param_handles`, not fields.
     - Unlinked corners (`rectangle-corner-radii`): the bound survives (radii on
       one side sum to at most that side, the map is linear, so any two
       neighbours on that side stay at least 14 px apart); that spec re-runs
       the clearance property over its domain, and checks edge and centre
       clearance for `ρ` up to 2.
  2. **Tiers 24 / 48 / 72: CONFIRMED.** With the normalised map clearance holds
     by construction for every `s ≥ T`, so `T` is no longer sized by the
     rectangle. The star sets it: gap `= (s/2)(√2 - ratio) - 10.66` px, at least
     4 gives `s ≥ 68.9`; 72 leaves 0.6 px. Code: one pure
     `handle_tiers(shorter_side, tolerances)` returning the four booleans, with
     the existing `reaches` slack (`THRESHOLD_SLACK`) for all three thresholds,
     so a box of exactly 72 px keeps its parameter handles under
     `px / scale * scale` rounding. The centre handle's drawn state is now a
     function of the parameter positions (hidden within 20 px of any of them,
     hidden while a parameter drag runs): one pure `centre_drawn(tiers, &params,
     centre, tolerances, param_drag_active)` used by the decoration only. The
     hit function never sees it (hover-only, never hit-tested), so no tier
     change reaches the hit rule. Refinements tier tests (24 and 48) stay
     unchanged and get a third boundary at 72 (71.9, 72, rounding slack); the
     existing order test gains "parameter handles disappear first".
  3. **Parameter hit radius 12 px: CONFIRMED** (not `resize_radius`). Nearest
     centre still decides; rank 0 still wins an exact tie. Because a drawn
     glyph's circumscribed radius (5.66) is below half the minimum centre
     distance (7), a point inside any drawn glyph always goes to that glyph's
     handle even where hit areas overlap (the corner glyph's 16 px area and a
     knob 15 px away overlap, and the corner still wins at its own centre). A
     test asserts this for every drawn glyph. Consequence worth knowing: at `ρ`
     near 1 the box centre (about 10 px from each knob) is a radius-handle press;
     the body stays reachable halfway between the centre and each edge midpoint
     (nearest knob 12.7 px away at `s` 72, minimum `s/(4√2)`, so larger above;
     the square box is the worst case for any aspect; **wrong for a wide box,
     see the dated note at the end of this file**). This replaces my earlier
     "a press at the box centre is a move" assertion.
  4. **Parameter handles not drawn during other drags: CONFIRMED.** It is a
     decoration rule: the drawn set (`Session`/`select_view`) skips the
     parameter family while a move, resize, rotate or skew drag runs, and for
     two or more objects. No hit test runs during a drag and the frozen
     handle set of such a drag contains no parameter handle, so criterion 6
     ("not drawn, no hit area") and the one hit rule are untouched. During a
     parameter drag the transform handles stay drawn and are frozen as before.
  5. **Clearance property test, star case: CONFIRMED with two corrections.**
     (a) The worst case is not "a multiple of 4". The handle sits at angle
     `θ + π/N` from the stored first-outer-vertex angle `θ` (`StarFrame::angle`
     is the drag direction, any value), and the box is the circumscribed square
     (corners at `R√2`, 45° off the axes). Gap to the nearest corner glyph is
     smallest when the handle's angle is `45° mod 90°`, which every `N` reaches
     by some `θ`, and it falls with the ratio, so the infimum is ratio 0.99 on
     the diagonal: `R(√2 - 0.99) - 10.66 = 4.6 px` at `R = 36` (`s` 72).
     (b) A random sweep can miss the exact diagonal, so the property is stated
     as: for `s ≥ T`, `N` in the full range, ratio in 0.01 to 0.99, `θ` in `[0,
     2π)`, object rotation, aspect irrelevant (star box is square), and zoom, the
     measured gap from the inner knob to every other drawn glyph centre (corner
     resize, rotate; the centre glyph yields) is at least 4 px **and** at least
     the analytic bound above minus 1e-9; plus one fixed case at the worst
     point (`θ = π/4 - π/N`, ratio 0.99, `s = T`) asserting 4.6 ± 0.05. The
     rectangle property is the same shape: `s ≥ T`, aspect 1 to 8, `ρ` in 0 to
     1 including both ends, rotation, zoom; every pair of drawn glyphs (all
     transform handles and the four knobs, centre excluded) is at least 4 px
     apart, using circumscribed radii (knob 5, squircle 5.66, others from the
     design system). Glyph sizes live as named pixel constants in `ui-core` for
     the test, and a `render-core` test asserts the drawn glyph never exceeds
     them (the two crates share no code, so the duplicated number is guarded on
     both sides). The 0.6 px margin is thin: any increase of the knob or
     corner glyph needs `T` re-derived by that formula.
  6. **Bar: no new `Session` state.** `select_bar_state` stays a pure function
     of snapshots and selection; the new Radius field is one more field in
     `SelectBarState` (`radius: Option<BarValue<Length>>` over the effective
     radii of the selected rectangles, plus `limited: Option<Length>` carrying
     the stored value when it exceeds the effective one for a uniform
     selection) and `remove_rounding` becomes `shown` / `enabled` booleans
     (enabled: some selected rectangle has a stored radius above 0 by more
     than 1e-9 mm). "Shown when the selection contains the kind, acts on
     exactly those" is a pure `ids_of_kind(&[ObjectSnapshot], &ObjectSelection,
     kind)` filter used by `select_bar_state` and by the commands
     (`set_corner_radius`, `set_point_count`, `set_inner_ratio` already take an
     id list), so a mixed selection writes only to its matching objects, in one
     commit. The Radius field is typed-only (no preview), so `bar_preview` and
     `ParamValue` gain nothing; open text, the invalid state and "Escape
     restores" live in the DOM like today's Points field. One wasm call, in
     `wasm_select_bar.rs`, takes the string and parses it with the existing
     `parse_entry_number` (one parser for chip and bar, locale handling in one
     place) and returns the `EntryOutcome`. One small rule for a batch: the
     value is limited to the **largest** half-shorter-side among the target
     rectangles and the one limited value is written to all (the register is raw
     and clamped on evaluation, `set_corner_radius` takes one value); a single
     rectangle is criterion 18 exactly, and a smaller rectangle shows its
     effective radius with the "limited" tag.
  7. **Two PRs, one window.** Unchanged and now stricter: between PR 1 and PR 2
     the Select bar has the Radius field and the new visibility rule while the
     shape bars still carry Remove rounding, Points, Ratio and Object to path
     under the old rule, and a double-click on a primitive still opens its tool.
     Merge PR 2 in the same release window as PR 1; PR 1 is not to be released
     alone.

- **2026-10-06: one resolving function per parameter, shared by drag, typed
  entry and bar field.** Options: (A) `ParamValue { Radius(Length),
  Ratio(InnerRatio), PointCount(PointCount) }` and one
  `apply_param(start, value) -> PrimitiveSnapshot` that holds the clamps
  (radius to half the shorter side, ratio to 0.01 to 0.99); a drag turns the
  pointer into a value, an entry turns text into a value, the bar turns its
  control into a value, and all three call `apply_param`; chosen. (B) The
  refinements pattern of feeding a synthetic pointer into the drag function.
  Rejected here: it works for radius and ratio but not for the arcs and the
  curve of the next spec, whose values are absolute (an angle from the pointer
  direction, a projection on a bisector), so a synthetic delta would need an
  inverse per kind. Under (A) the later kinds add a `ParamValue` variant and a
  `value_from_pointer` arm, nothing else.
  - A param drag is the existing `TransformDrag` with `handle:
    EditHandle::Param(_)`: `resolve` gains one arm, `commit_gesture` gains one
    arm (`commit_param`: `set_corner_radius(&[id], r)` or
    `set_inner_ratio(&[id], ratio)`, the existing commands). No second drag
    struct, no second dead-zone test. `DragOrigin` already records Shift at the
    press (as `side_rotate_revealed`); it is renamed `shift_at_press` in the
    rename commit so `rectangle-corner-radii` can read it for "Shift inverts the
    link".
  - Preview and release are `resolve` with the same arguments (criterion 13).
    `resolve` returns the start snapshot for a non-finite input, as for resize.

- **2026-10-06: how the shape-tool drag code migrates, and the radius rule
  (customer decision, switch default off).**
  - Old to new, one rule each:

    | Old (shape tools) | New |
    |---|---|
    | `RectangleTool` radius drag, `corner_radius_from_drag` | `value_from_pointer` for `CornerRadius`, same arithmetic; one handle per corner, each projecting on its own inward diagonal |
    | `PolygonStarTool` inner-radius drag, `inner_ratio_from_drag` | `value_from_pointer` for `InnerRadius`, same arithmetic |
    | Rect, ellipse resize: `resize_rect_bounds`, `resize_ellipse_frame`, `pin_rect_resize`, `pin_ellipse_resize` | deleted; `resize_by_local_delta` is the only resize rule |
    | Polygon/star resize on four cardinal handles: `scale_star_frame` | deleted; `polygon_star_resize_factor` on corner handles is the only rule (criterion 16 keeps it) |
    | `RectangleTool::remove_rounding` | `ParamValue::Radius(0)` applied to every selected rectangle through `commit_param_batch` (`set_corner_radius(&ids, 0)`) |
    | `PolygonStarTool::set_point_count`, `set_ratio`, `preview_ratio`, `commit_ratio_preview` acting on the selection | the bar's `ParamValue::PointCount` / `Ratio` through `LiveEdit`; the tool keeps only its next-shape settings (criterion 29) |
    | `local_delta(rotation, a, b)` | already `local_delta_of` on the box; deleted |

  - **The two radius rules.** Slice 5 scaled the radius by `√(sx·sy)` in the
    Select tool (the factor `resize_primitive` already computes for the stroke);
    the Rectangle tool kept it absolute. After this feature the Rectangle tool
    cannot resize, so only one rule remains: `ScaleModes { stroke:
    StrokeScaling, radius: CornerRadiusScaling }`, with `CornerRadiusScaling {
    Keep (default), Proportional }` (an enum, not a bool: `StrokeScaling`'s
    reason). `SelectTool` holds the pair; `TransformDrag` captures it at the
    press; `TransformEntry::for_resize` captures it when the entry opens. In
    `resize_primitive` the radius factor is `stroke_or_radius_factor(sx, sy)`
    for `Proportional` and exactly 1 for `Keep`. Rotated rectangles, Shift and
    Ctrl change nothing about this.
  - **`resize_*` commands and `Option<Length>` (the PO's question).** No
    signature change is needed. `Document::resize_rect` already compares the
    passed radius with the stored one and skips the write when they are equal
    (`resize_rect` in `shapes.rs`: "an LWW rewrite of an unchanged value would
    be a new operation that could beat a concurrent radius edit"). Under `Keep`
    the resolved snapshot carries the stored raw radius unchanged, so the
    register is not written, which is the property `StrokeScaling::Keep` gets
    from `Option<Length>`. The stored radius is raw: a rectangle shrunk below
    `2·r` keeps its stored radius and only the effective one shrinks, and
    enlarging brings it back (the `0003` rule). A merge test pins it: peer A
    resizes with `Keep` while peer B sets the radius; B's radius survives.
    `specs/0013-rectangle-corner-radii/` keeps this per register when it makes the
    radius four registers.
  - **Behaviour change for the lead's list:** the default result of a
    Select-tool resize of a rounded rectangle changes (radius no longer scales).
    The `0005` criteria 9 and 31 tests that assert `√(sx·sy)` run with the
    switch on and keep their numbers; new tests assert the off default.

- **2026-10-06: the single preview mechanism: blue new over black old, from
  one resolved snapshot.**
  - `SelectTool::live_edit(objects, selection, pointer, modifiers) ->
    Option<LiveEdit>`; `LiveEdit` holds the resolved `Vec<ObjectSnapshot>`: for
    a move every selected object through `ObjectSnapshot::translated` (the rule
    `translate_objects` commits with), for a transform or parameter drag the
    one `resolve` result, for a bar edit `apply_param` over the selected
    primitives. It returns `None` while the drag is inside the dead zone, and
    also when the resolved objects equal the committed ones within 1e-9 mm and
    1e-12 rad, so "a drag that returns to its start point" shows nothing
    (criterion 12). `pointer_up` commits the same resolved objects. The old
    `live_offset`, `live_transform` and their callers collapse into this one
    function (they move to `select_tool/preview.rs` with it).
  - `Session` stops substituting live geometry into what it draws: the Select
    branches of `primitives_for_render` and `live_node_drag_paths` are deleted
    (the Node tool's own node-drag substitution stays: question 6 is not in this
    spec), and `primitives_for_render` itself goes, because its remaining job
    (the slider's ratio) is now a `LiveEdit`. The strokes of paths and
    primitives are always the committed document, so "black old" is not a
    second render path but the absence of one. The blue layer is
    `build_live_edit_preview(&live.objects, view)` in `render-core`, which
    generalises today's `build_shape_live_preview` to any `ObjectSnapshot` (a
    primitive through `outline_of_rotated`, a path through its anchors, both
    through `stroke::path_stroke` with `theme::ACCENT` and
    `LIVE_PREVIEW_STROKE_PX`, hollow). The selection box, the handles, the
    pivot marker and the readout are built from the live objects exactly as
    today (`select_view::live_objects`, now the only substitution point and
    used by decorations only).
  - Preview and commit share the resolving function by construction (criterion
    13), and Shift and Ctrl changes with the pointer still reach it because
    `draw_list` already reads the cached modifiers at render time
    (`modifiers_changed`, criterion 14).
  - **Hole closed.** Today a move drag that left the dead zone and returned to
    its start commits `translate_objects` with a zero offset, which rewrites
    every anchor or frame register with its own value. The move commit now
    returns before writing when the offset is within 1e-9 mm of zero. Test
    first.
  - **Cost (criterion 15).** The overlay tessellates only the moved objects, a
    second time per frame, on top of the full rebuild the renderer already does
    (`docs/technical-debt.md`, "renderer not cached"). Reference desktop: the
    customer's Linux machine under Tauri/WebKitGTK, WebGL2. The gate is a
    `#[ignore]` benchmark of `Session::draw_list()` for the 200-object move,
    run in release by the tester, with the budget 8 ms per frame of CPU time
    (half of a 16.7 ms frame; 50 fps needs 20 ms); the number goes into the PR
    and the demo, plus one frame-rate check on the real window. If it fails,
    the fix is caching the committed draw list by (document version, scale),
    the entry's own resolution, not a second preview path.
  - **Slider and number fields (criterion 21) use the same overlay.** A
    pending Points or Ratio edit is `SelectTool::bar_preview: Option<ParamValue>`
    plus the ids it was started against (the existing flush-before-selection-
    change rule of `commit_poly_star_ratio` moves here: `set_tool`,
    `pointer_down` and a selection change call `flush_bar_preview`). Criterion
    10 lists drags; this renders the slider the same way so there is one
    mechanism, no second code path. The PO may veto and keep the slider
    black-only; the cost of that is a branch in `live_objects`.

- **2026-10-06: the Select bar's controls and the state they need.**
  - Pure `select_bar_state(&[ObjectSnapshot], &ObjectSelection, bar_preview)
    -> SelectBarState` in `select_bar.rs`, the counterpart of
    `NodeToolbarState`: `remove_rounding: bool` (selection is only
    rectangles), `object_to_path: bool` (flag 6), `points: Option<BarValue<u32>>`
    (only polygons and stars), `ratio: Option<BarValue<f64>>` (only stars),
    with `BarValue { Uniform(v), Mixed }` (equality of ratios within 1e-9).
    The two switches are not in it: they are session state with accessors
    (`scale_stroke_width`, new `scale_corner_radius`), operable with no
    selection.
  - Session and wasm: `remove_corner_rounding()` (now Select-only),
    `set_selected_point_count(u32)`, `set_selected_ratio(f64)` (discrete
    commit), `preview_selected_ratio(f64)` / `commit_selected_ratio()` (slider),
    `select_bar_state()`, `scale_corner_radius()` / `set_scale_corner_radius(bool)`
    (closes an open entry, like the stroke switch), `convert_selected_to_paths()`
    unchanged. New wasm methods go in a second `#[wasm_bindgen] impl
    WasmSession` block in `wasm_select_bar.rs` (the `session` field becomes
    `pub(crate)`); `wasm_api.rs` (772 lines) does not grow.
  - Frontend: `SelectToolbar` takes the state and callbacks; `ScaleStrokeSwitch`
    is generalised to one `ToolbarSwitch` used twice (two concrete users now);
    `ShapeToolbar` loses Remove rounding and Object to path in PR 2. The state
    machine for a slider drag is the one `ShapeToolbar` has today (preview per
    tick, commit on pointer up, key up, blur).

- **2026-10-06: typed entry on a parameter handle.** `ParamEntry` (one field,
  `param_entry.rs`) next to `TransformEntry`; `SelectTool::entry` becomes
  `Option<OpenEntry>` with two variants (two concrete types use the enum).
  Parsing is the existing `parse_entry_number`; the outcome is the existing
  `EntryOutcome` (`Committed`, `Unchanged`, `Invalid`). A value above the radius
  limit or outside 0.01 to 0.99 for a ratio is limited and the limited value is
  written for the radius (criterion 18) and refused for the ratio (criterion 19
  says invalid); both go through `apply_param`, so drag and entry cannot
  disagree. Unchanged rules: untouched text, equal value within 1e-9 mm, and
  every close path of the refinements spec write nothing. The double-click path
  is the existing "first press must have grabbed the same handle" rule
  (`last_press_handle`). `EntryKind` gains `CornerRadius` and `InnerRatio`;
  the existing `Radius` (a polygon or star's outer radius) is renamed
  `OuterRadius` in the rename commit (flag 4).

- **2026-10-06: what the shape tools keep and what is deleted.**
  - Kept: the three tools, `pointer_down(point)` / `pointer_move` /
    `pointer_up` / `escape`, the create-drag state, `live_shape` for the create
    preview and its readout, `constrained_endpoint`, `is_degenerate`
    (and `create_drag_box` when `shape-creation-from-center` lands), the
    polygon/star tool's `mode`, `point_count`, `ratio` as next-shape settings.
    A press with no movement writes nothing and does not touch the selection
    (criterion 26: `apply_selection_click` is gone).
  - After a create-drag `Session::shape_pointer_up` sets `Tool::Select` and
    selects the new id in the same call (criterion 28); the frontend already
    re-reads `session.tool()` in `syncFromSession` after each call (and reads
    `select_bar_state()` there too). The rail highlight follows.
  - Deleted (dead code must go): `handle_layout.rs` except `ResizeDirection`
    (moved to `resize_direction.rs`; `HandleKind`, `ShapeHandle`, `handles_for`,
    `rect_handles`, `ellipse_handles`, `polygon_or_star_handles`,
    `corner_inward_diagonal`, `resize_rect_bounds`, `resize_ellipse_frame`,
    `scale_star_frame`, `ResizeDirection::CARDINAL_FOUR`; the two `*_from_drag`
    functions reborn in `param_edit.rs`, the inner handle position in
    `param_handles.rs`); `shape_hit_test.rs` (`hit_test_handle`,
    `hit_test_primitive`); `ShapeHitTolerances`; `LiveShape::Adjusting` (the
    enum becomes a struct `CreatePreview`); `rects_only`, `ellipses_only`,
    `polygons_and_stars_only`, `apply_selection_click`; `pin_rect_resize`,
    `pin_ellipse_resize` (`pin_resize_anchor` stays); the three tools' `Resizing`
    and `Dragging*` states; in `Session`: `hovered_primitive`,
    `update_hovered_primitive`, `shape_matches_active_tool`, `tool_for_shape`,
    `shape_decoration_input`, `primitives_for_render`, the `remove_corner_rounding`
    gate on `Tool::Rectangle`; in `render-core`: `ShapeDecorationInput`,
    `RenderShapeHandle`, `ShapeHandleKind`, `shape_handle_glyph`, the primitive
    bounding-box outline in `shape_preview.rs` (`build_shape_draw_list` keeps only
    primitive strokes and is renamed `build_primitive_strokes`); in `wasm_api.rs`
    the old `poly_star_*` selection-editing calls; in the frontend the
    shape-tool bar's selection controls. A creation tool with a selection draws
    each selected object's plain `SelectDecorationInput` box with no hover and
    no handles (criterion 27); `select_decoration_input` serves both.
  - Closing check for PR 2: `cargo clippy` with `dead_code` denied (already
    in the gate) plus a grep for each deleted name in the PR description.

- **2026-10-06: tests that change.** None is weakened or deleted without a
  replacement that asserts at least as much. Rule: **port in PR 1, delete in
  PR 2.** PR 1 adds the Select-tool equivalent of every shape-tool test it
  supersedes, copying the old test's numbers; PR 2 deletes the old one and names
  its replacement in the PR description.
  - Ported, then deleted in PR 2: `rectangle_tool.rs` unit tests (resize,
    radius drag including rotated and shrunken, zero at the diagonal, remove
    rounding, selection click); `ellipse_tool.rs` and `poly_star_tool.rs` unit
    tests (resize, inner radius, point count live, ratio preview and flush);
    `handle_layout.rs` and `shape_hit_test.rs` tests;
    `curvyo-ui-core/tests/acceptance_0003.rs` AC3, AC9, AC13, AC14 (both),
    AC15, `hit_test_handle_ignores_non_draggable_echo_handles` (replaced by
    "all four radius handles are draggable"); `session/shapes.rs` tests
    `corner_radius_drag_renders_a_live_preview_through_the_session`,
    `ratio_slider_preview_renders_live_but_commits_once`,
    `ratio_preview_flushes_against_the_previewed_star_not_a_newly_selected_one`.
  - Rewritten in place (assertion strength kept): the `0005` resize tests that
    assert the radius scales by `√(sx·sy)` (switch on, plus new off-default
    tests); the tests that read the Select preview as "the object redrawn at the
    new geometry" (they assert black original unchanged plus a blue layer equal
    to the committed result); `session/shapes.rs`
    `live_readout_only_applies_to_a_create_drag` (create-drag or Select edit);
    `ac17_ac21_*` and `ac22_*` conversion tests (driven from the Select bar's
    path); `curvyo-editor-wasm/tests/fix_select_created_shape.rs` (the tool is
    now Select after a create-drag; the draw-list assertion stays).
  - Double-click handoff for a primitive, to a negative of equal strength (tool
    stays Select, nothing written, selection unchanged), the path handoff
    untouched: `acceptance_0004.rs` (22, 23), `acceptance_0005.rs` (the
    handoff blocks near lines 2418 to 2621 and 3057), `acceptance_otr_dblclick.rs`,
    `acceptance_otr_tester.rs` (about 589 to 638),
    `acceptance_object_transform_refinements.rs` (about 326 to 330 and 607),
    `reverify_0004_stale_id.rs`, and the `Tool::Rectangle` setups in
    `acceptance_0004.rs` that drew handles (about 502 to 567, 622 to 659).
  - New, test first: handle tiers and the 4 px clearance property, hit order
    with ties, `apply_param` clamps, drag equals entry equals bar, zero-offset
    move, radius `Keep` merge test, preview equals commit for every handle kind,
    creation tools never select or draw handles, format round trip (38).

- **2026-10-06: the two-PR split, task order and exact cut line.** Both PRs
  are `story/unified-object-editing`-derived (`story/unified-object-editing`
  then `story/unified-object-editing-creation-only`); each needs customer
  acceptance (`CLAUDE.md` §9). The story is done when both are in.

  **PR 1: criteria 1 to 24, 35 (without the advanced-selection parts), 37,
  38.** Adds; deletes nothing the shape tools use.
  1. Pure-move prelude, one commit, no behaviour change: `resize_primitive`,
     `scaled_star_frame` and `pin_*` from `transform_drag.rs` to
     `transform_primitive.rs`; `SkewFrame`, `skew_frame`, `skew_angle`,
     `skew_factor` from `transform_math.rs` to `skew_math.rs`; the preview
     accessors from `select_tool.rs` to `select_tool/preview.rs`.
  2. Rename commit: `TransformHandle` to `EditHandle` (empty `Param` variant),
     `DragOrigin::side_rotate_revealed` to `shift_at_press`, `EntryKind::Radius`
     to `OuterRadius`. Compile-driven.
  3. `param_handles.rs` layout (`L`, inset, pitch), tolerance fields,
     `handle_tiers` with thresholds 24/48/72, `centre_drawn`, drawn-set rule for
     other drags, both clearance properties incl. the star worst case (1, 4, 6,
     7, 8).
  4. Param arm in `hit_transform_handle`; tie and shrink tests (5, 6, 35).
  5. `param_edit.rs` with ported tests: `ParamValue`, `value_from_pointer`
     (gain frozen in the drag at the press),
     `apply_param`, `commit_param`; `TransformDrag` arms; dead zone and Escape
     (2, 3, 4, 9).
  6. `ScaleModes`, `CornerRadiusScaling`, `resize_primitive` radius factor,
     `Keep` merge test, session accessors (23; the radius part of 0005 AC 9, 31).
  7. `LiveEdit`, `build_live_edit_preview`, `Session` substitution removal,
     zero-offset move guard, the release-equals-preview property test for every
     handle kind, the 200-object benchmark (10 to 15).
  8. `ParamEntry`, readouts ("r 3.5 mm", "ratio 0.45"), cursor and hint strings
     (18, 19, 20).
  9. `select_bar.rs` (`ids_of_kind`, Radius field state, contains-the-kind
     rule), `wasm_select_bar.rs`, Select bar controls, second switch, frontend
     (21, 22, 23). Remove rounding, Points, Ratio and Object to path
     exist in both bars until PR 2.
  10. Round-trip and compatibility tests (24, 38), full gate, `docs/design-system.md`
      rows from the `ux-engineer`, demo.

  **Cut line: after task 10.** PR 1 ends with the Select tool complete and the
  old shape-tool editing still working. In between the two PRs a rectangle can
  be edited in both places and a double-click on a primitive still opens its own
  tool, so the lead merges PR 2 in the same release window (a requirement: PR 1 is
  not released alone).

  **PR 2: criteria 25 to 34 and the PR 2 half of 38.** Delete-and-rewire.
  1. Creation-only tools: no selection, hover, handles or resize; press without
     movement writes nothing and keeps the selection (25, 26, 27).
  2. After a create-drag the Select tool is active with the new shape selected
     (28); frontend tool sync.
  3. Polygon/star bar is settings-only; Rectangle and Ellipse have no bar
     controls (29, 30); delete the selection-editing wasm calls.
  4. Double-click on a primitive does nothing; `SelectDoubleClickOutcome::Hit`
     is for paths, `tool_for` is path-only (31 to 34).
  5. Delete the dead code listed above, port the remaining tests, grep check.
  6. Gate, docs (`docs/design-system.md`, the owning specs the lead updates),
     demo.

- **2026-10-06: size limits (`docs/technical-debt.md`, "`Session` is one
  module past the size limit").** After this feature: `select_tool.rs` below 500
  non-test lines (546 now, minus about 110 moved out in PR 1 task 1, plus about
  30 for the press order; the new work goes into `select_tool/preview.rs`,
  `param_*.rs`, `select_bar.rs`); `transform_drag.rs` about 380 (505 minus the
  prelude); `transform_math.rs` about 430 (527 minus skew); `transform_entry.rs`
  does not grow (the param entry is its own file, 490 lines are all non-test);
  `wasm_api.rs` does not grow (new calls in `wasm_select_bar.rs`, PR 2 deletes
  about 80 lines); `session/shapes.rs` falls to about 150 non-test lines in PR 2;
  `rectangle_tool.rs` 370 to about 90, `poly_star_tool.rs` 486 to about 190,
  `ellipse_tool.rs` 240 to about 90. Each new module has a one-sentence doc
  comment without "and". `advanced-selection` still has to take its own
  pressure off `select_tool.rs`; this feature leaves it more room, not less.

- **2026-10-06: purity and wasm32.** All new code is in `curvyo-ui-core` and
  `curvyo-render-core`: no filesystem, network, clock, thread or UI; both
  crates keep `#![forbid(unsafe_code)]` and build for `wasm32-unknown-unknown`
  in the gate. The benchmark reads the clock only inside an `#[ignore]` test.
  Dependency direction is unchanged: `render-core` takes `ObjectSnapshot`
  (document-core), never a ui-core type; the glyph kind is its own enum.

- **2026-10-06: room for the two follow-up specs, and where the shared Curve
  function goes.**
  - `rectangle-corner-radii`: `CornerRadius(Corner)` is already four handles;
    its change is `value_from_pointer` writing one corner and `apply_param`
    taking a corner, plus `Document` growing four registers (its own ADR note and
    `format_version`). `resize_rect` keeps skipping unchanged registers, per
    register, so "Scale corner radius" off still writes none. Link and Shift are
    read from `ScaleModes`-style session state and `DragOrigin::shift_at_press`.
  - `ellipse-arcs-and-shaping`: `ArcStart`, `ArcEnd`, `Curve` join `ParamHandle`;
    their `ParamValue` variants carry an angle and a curve newtype; their
    `value_from_pointer` arms use the absolute pointer in the shape's local
    frame (decision on param drags above). The shared Curve evaluation (ring of
    vertices, side arc, `k_min`, side middle, cubic control points) belongs in
    `curvyo-document-core` next to `primitive_outline.rs` (a new sibling
    module): outline, hit test, render, "Object to path" and the handle
    position all already reach geometry through `outline_of`, and `ui-core`
    positions the handle from the same function, so criterion 51 of that spec
    holds by construction. That spec's `adrs.md` decides it; nothing here
    forecloses it.
  - Neither follow-up needs a change to the registry shape, the hit rule, the
    drag struct, the preview or the entry enum.

- **2026-10-06: sequencing, and which criteria this spec supersedes.**
  Recommended order, with reasons:
  1. **This feature, PR 1 then PR 2.** It rewrites the press dispatch and
     removes the code the next two specs would otherwise edit and then replace.
  2. **`shape-creation-from-center`.** Small, Must, independent of the Select
     tool once its criteria 16 and 17 are replaced; building it after PR 2
     means it edits creation-only tools (no handle code) and its
     `create_drag_box` is then the only create computation.
  3. **`rectangle-corner-radii`.** The customer's own request from the same
     remark; it has the first stored-field change after this feature, so it
     takes the next `format_version` and coordinates with `0007`.
  4. **`0007-stroke-and-fill-styling`.** Its overlap with this feature is the
     resize commit and the stroke register; waiting until after PR 2 means the
     four resize commands are already the only resize call sites (the typed entry
     and the Select drag), so re-pointing `stroke_width` is done once.
  5. **`advanced-selection`.** Planned after `0007` already (its ADR); it
     calls `handle_at` as a finished function over the full handle set
     including parameters, and its Alt, Shift and Ctrl rules arrive in a press
     state machine that already has one handle family.
  6. **`ellipse-arcs-and-shaping`.** Largest; two model changes and the shared
     Curve function; builds on 1 and 3 and shares the `format_version`
     coordination. The lead may move it ahead of 4 and 5 if the customer ranks
     it higher; nothing here depends on 4 or 5.
  No two run in parallel (the PO's rule, and they share `select_tool`).

  Criteria superseded elsewhere, exactly:
  - `shape-creation-from-center` **criterion 16** (a press on an existing shape
    of the tool's kind selects or handle-drags; Shift toggles) is replaced by
    unified 25 and 26. **Criterion 17, first sentence** (handle drags "under its
    own tool") ends with the tools' editing role; its second part (the Select
    tool's modifiers, the polygon/star create-drag unchanged) stays. **Out of
    scope bullets** "Modifier behaviour on shape-tool handle drags" and "Fixing
    that a newly created shape is not selected after release" are stale (the
    second was fixed by PR #34 and is now unified 28). Criteria 1 to 15 stand.
  - `advanced-selection`: no numbered criterion is replaced. Its ADR notes
    change: (a) "the shape tools keep 4 px" is moot (`segment_tolerance` is used
    by the Node and Select tools only; the shape tools have no hit test),
    (b) the double-click order "handle, inside the sole selected box (handoff),
    outline" applies to paths only (primitives do nothing, unified 31 to 34),
    (c) "the shape tools ignore `alt`" is trivially true. Unified criterion 35's
    Alt, Shift-arm and Ctrl-arm clauses name `advanced-selection` criteria 16 and
    17, which are not built yet: they take effect when that spec is built
    (flag 3).
  - `0003-primitive-shapes` and `0004`, `0005`, `object-transform-refinements`:
    as the unified spec's "Changes to accepted behaviour" lists (the lead
    updates them on acceptance).
  - `0007-stroke-and-fill-styling`: only the dated note that the four resize
    commands remain the single resize call sites.

## Flagged to the lead

1. **Criteria 2, 7 and 8 cannot all hold as written.** A radius handle "at
   radius 0" sits exactly on the corner resize glyph (criterion 8 forbids
   overlap), and at the largest radius the four handles meet (for `d0` 12 px at
   slope 1 they clear each other by 4 px only from a shorter side of about
   100 px, not 64). **Resolved 2026-10-06 (second pass): the UX normalised map (15 px inset, far
   end `(s-14)/√2`, gain) gives 4 px clearance from `T` = 72, so criterion 7's
   number becomes 72 (PO edit).** Original default: the fixed screen inset `d0` and slope 1
   (decision above); `T` is the `ux-engineer`'s value and the clearance
   property test fixes the minimum. Alternative if 64 px matters more than a
   handle that tracks the pointer: a compressed position map with an inverse in
   the drag. Not built unless asked.
2. **Criterion 8 for the star's inner handle.** The ratio runs 0.01 to 0.99, so
   the handle can sit on the centre glyph. **Default:** the centre glyph (hover
   only, never hit-tested) yields: it is not drawn when within 4 px of a
   parameter glyph. The PO or `ux-engineer` should accept this reading of "no
   two drawn glyphs overlap".
3. **Criterion 35 names `advanced-selection` criteria 16 and 17**, which do not
   exist in code until that spec is built. Until then the order is handle, move
   inside the box, outline select, empty press clears; the Alt and arm clauses
   are a conditional the later spec fulfils.
4. **Accessible name collision, criterion 18.** "Radius" is already the
   accessible name of a polygon or star's outer-radius entry. **Default:**
   "Corner radius" for the rectangle, "Outer radius" for polygon and star,
   "Inner ratio" for the star (labels stay "r", "r", "ratio"). PO to confirm.
5. **Criteria 23 and `rectangle-corner-radii` criterion 14** differ in wording
   (read at the press or at Enter). They agree in effect: the switch is read
   when the entry opens, and clicking it closes the entry without writing
   (refinements criterion 31). The PO should reword criterion 14.
6. **Criteria 21 and 22 disagree on when a control shows.** 21 says "consists
   only of", 22 says "one or more primitives". **Default (superseded by the UX rule: a control shows when the selection
   contains its kind and acts on those objects only):** Remove rounding,
   Points and Ratio need an all-rectangle, all-polygon-or-star, all-star
   selection respectively; Object to path shows when the selection holds at
   least one primitive and converts only those (the current
   `build_primitive_conversions` filter), and the tool switches to the Node tool
   afterwards as today. PO to confirm.
7. **Criterion 7's fallback is thin for rectangles.** Below `T` the radius of a
   rectangle can only be set by zooming (the bar offers only Remove rounding).
   Recommend a "Radius" number field in the Select bar for rectangles, parallel
   to Points and Ratio; `rectangle-corner-radii` would then define it as "set
   all four". Default: not added, because the spec does not list it. **Update: the UX
   notes specify the field; buildable with no new state (second-pass note,
   item 6); scope is the lead's/PO's call, default include.**
8. **Hole in criterion 12 for moves**, fixed by decision 5 (a zero-offset move
   commit rewrote registers). No spec change needed; the tester should assert it.
9. **Two states between the PRs** (decision on the cut line): shippable but
   inconsistent, so merge PR 2 in the same window.
10. **Frontend has no test runner** (`docs/technical-debt.md`, "The quality gate
    covers only half the product"). Everything testable is in `ui-core` and
    `Session`; the bar, switch and chip are covered by the `ux-engineer` review
    and the demo, not by an automated test.

- **2026-10-07 (architect, `rectangle-corner-radii` review): the body-reachability
  claim of the second-pass note and the "quarter of the shorter side" sentence
  of the layout decision are false as stated.** A point halfway between the
  centre and an edge midpoint can be within the 12 px hit radius of a knob,
  and the square is not the worst case: on a box of aspect 1.8 with one uniform
  radius at `ρ` = 1 a knob sits 7.7 px from the halfway point of the long edge.
  What holds: a press is a handle if a drawn handle's centre is within its
  radius (parameter handles 12 px, no inner band; resize handles only inside a 6
  px band); the nearest centre wins, ties go to the parameter handle, then TL,
  TR, BR, BL; every other point inside the box is a move. The dead zones are the
  four 12 px discs and the 6 px bands; over the scanned configurations at least
  35 to 41 % of the interior stays a move, and the centre of a near-square at
  `ρ` about 1 is a knob press in about 9 % of random states. This is accepted
  (the centre handle and the Move cursor remain the move routes). The
  glyph-clearance property (4 px) is unaffected.
