# ADRs for "Stroke and fill styling: width/dash/join/cap/color, solid and gradient fill"

This slice extends `path-node-editing` (slice 2) and `primitive-shapes`
(slice 3). It reopens no decision from either. It replaces the placeholder
style both slices store (`stroke_width`, `stroke`, an unstored `fill`) with
one style schema that every object node carries, whatever its `shape`. That
is the document model again, so the schema is written out in full below.

All 25 acceptance criteria can be built against ADR 0002 §5 and ADR 0009 §3
as they read today. **No new crate, no new external dependency, no ADR
amendment.** `vecmanf-geometry-core` gains one function, for AC 23's
interior hit-testing. The draft's one conflict (AC 9's dash storage) and
its two open points (AC 16's stop count, interior selection) were resolved
in the specification on 2026-10-04; see "Flagged to the lead".

**Build order:** this slice touches `vecmanf-document-core`,
`vecmanf-render-core` and `vecmanf-ui-core`. `primitive-shapes` and
`path-merge-split-and-node-types` are both merged. **2026-10-05 (architect):
`object-transform` (slice 5) also touches these same crates and this
slice's `style: Style` replaces the `stroke_width` field slice 5's resize
(AC 8) writes directly — a real collision, not just a shared-crate caution.
This slice starts after `object-transform` merges** (`CLAUDE.md` §4). Once
that lands, `path_topology.rs`'s `split_at_anchor` must copy `style` with
fresh `StopId`s for its new object, alongside slice 5's `rotation` copy
already there.

## Depends on

- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  "each node carries ... its own fully resolved style: no CSS cascade, no
  inheritance". That is why the style keys sit on the object node itself,
  and why every gradient is a per-object copy. The specification's "no
  shared gradient definitions" is §5, not a simplification.
- [ADR 0002 §2, §4](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  stroke width is a `Length`. Opacity, stop position and dash entries are
  validated newtypes, not bare `f64`.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one interaction is one commit. Slice 2's command funnel (one `Document`
  method per command, ids resolved before the first write, one labelled
  Loro commit) is extended.
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): the
  live preview of a colour-picker or slider drag, the selected gradient
  stop, and any panel state are ephemeral. A selected stop is held as a
  `StopId` and resolved lazily.
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md):
  "Field granularity makes 'one peer changes the fill, another the stroke
  width' conflict-free". The register split below is that sentence applied
  per control. Gradient stops are the anchor pattern: a movable list of
  maps.
- [ADR 0003 §1, §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md)
  and [ADR 0001 §4](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  dashes, joins, caps, fills and gradients here are display tessellation at
  display tolerance, which is `render-core` with `lyon`. The kernel is not
  involved.
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md):
  `render-core → document-core` only. That edge decides the rendering
  boundary below.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  forces the `format_version` bump below.
- [`specs/0002-path-node-editing/adrs.md`](../path-node-editing/adrs.md) and
  [`specs/0003-primitive-shapes/adrs.md`](../primitive-shapes/adrs.md): caller-
  minted ids passed into `document-core`, "commands carry resolved data",
  "a press and release with no pointer movement writes nothing", open-file
  validation that dispatches on `shape` and tolerates unknown keys, and
  `primitive_outline` as the one outline every consumer uses. All apply
  unchanged.

## Deliberately not in scope for this slice

- **SVG mapping** (slice 11). The schema is chosen so the mapping is direct:
  keys mirror `stroke`, `stroke-opacity`, `stroke-width`, `stroke-linejoin`,
  `stroke-linecap`, `fill`, `fill-opacity`, and gradient geometry is in
  `objectBoundingBox` units. Two things the exporter must do: multiply the
  dash ratios by the width, and write `stroke`/`fill` explicitly on every
  element, because our absent defaults (stroke on, fill off) are not SVG's
  initial values.
- **Manufacturing meaning of style** (slice 14). Nothing here is read by
  job generation. A dashed stroke stays a display effect. If a story ever
  needs dashed *cut* geometry (perforation), `geometry-core` gets its own
  dasher at manufacturing tolerance. ADR 0003 §7 says display and
  manufacturing must not share a tolerance.

## Feature-local decisions

- **2026-10-04: the style schema.** Same keys on every object node in the
  `"paths"` tree, path or primitive, independent of `shape`. AC 1 and AC 2
  hold by construction: no key depends on the object kind, and no style key
  is a shape-parameter key.

  ```text
  object node, style keys (each its own LWW register unless noted):
    stroke_enabled : bool                         absent = true     (AC 5)
    stroke_width   : f64 mm, > 0                  existing key      (AC 4, 5)
    stroke         : [r, g, b]                    existing key      (AC 6)
    stroke_opacity : f64 in [0, 1]                absent = 1.0      (AC 6)
    stroke_dash    : [f64 …] multiples of width   absent = []       (AC 7–9)
                     even length, each ≥ 0, sum > 0; [] = solid
    stroke_join    : "miter" | "round" | "bevel"  absent = "miter"  (AC 10, 11)
    stroke_cap     : "butt" | "round" | "square"  absent = "butt"   (AC 12)
    fill_enabled   : bool                         absent = false    (AC 13)
    fill_kind      : "solid" | "linear" | "radial" absent = "solid" (AC 13, 21, 22)
    fill           : [r, g, b]                    absent = black    (AC 14)
    fill_opacity   : f64 in [0, 1]                absent = 1.0      (AC 14)
    fill_stops     : movable list of stop         absent until the first
                                                  gradient          (AC 16–20)
  stop = map:
    id       : hex string of the StopId (u128)    written once
    position : f64 in [0, 1]                      LWW register
    color    : [r, g, b]                          LWW register
    opacity  : f64 in [0, 1]                      LWW register
  ```

  **Absent defaults are part of the format and frozen.** They equal what
  slices 2 and 3 render today: stroke on, black, 0.25 mm (written at
  creation, as now), opaque, solid, miter join, butt cap, miter limit 4 (the
  `lyon` `StrokeOptions` defaults the current `stroke.rs` uses), and no
  fill. Every version-3 object therefore renders unchanged (AC 3) with no
  migration. A later default for *new* objects (for example the PO's open
  "remember last style" item) is written explicitly at creation, and it
  never changes what an absent key means. That open item is UI and
  preference state either way. It does not touch this schema.

  The slice-2 review note reserved `fill` for "None". Nothing ever wrote
  it. It now holds the solid fill colour, mirroring `stroke`. On/off is
  `fill_enabled`.

- **2026-10-04: register granularity. One register per control, plus an
  on/off flag for each paint.** This is the slice-3 rule ("values that one
  handle drag writes together are one register; values that separate
  controls write are separate registers") applied to the style panel:

  1. **Every stroke and fill property is its own register.** AC 4–14 give
     width, colour, alpha, dash, join, cap and fill kind separate controls.
     The specification makes colour and alpha "independently" set (AC 6,
     14). AC 24 requires that a change to one property leave "every other
     style property ... as it was". As separate registers, a concurrent
     colour change and dash change both survive, and every merged
     combination is one a maker could have chosen. **One grouped `stroke`
     register was rejected.** Every control would rewrite the whole group,
     and a concurrent edit to a different property would be lost. AC 24's
     multi-object edit would also become a read-modify-write per object
     instead of one key write.
  2. **On/off is a flag, separate from the values it gates** (AC 5, 13).
     Turning a paint off writes only the flag, so the stored values
     survive, and a concurrent colour change made while another peer turned
     the paint off is still there when it is turned back on. **Encoding
     "off" as a value was rejected** (`fill = none`, or width 0): it would
     overwrite exactly what AC 5 and AC 13 say must be restored. For the
     same reason, `fill_kind` stays separate from `fill_enabled`: "re-enabling
     a fill restores the last value" needs to remember *which* fill was last.
  3. **Entering width 0 is the "no stroke" command** (AC 5). It writes
     `stroke_enabled = false` and leaves `stroke_width` at its last positive
     value. So `stroke_width > 0` always holds, and AC 5's "previously set
     width ... kept stored" holds for both routes. Typing a non-zero width
     while the stroke is off is one command and one commit. It writes
     `stroke_width` and `stroke_enabled = true`, as the UX notes require.
  4. **The dash pattern is one register, and it stores multiples of the
     stroke width, not millimetres.** A pattern is chosen as a whole, so an
     element-wise merge could only produce a pattern no peer chose (ADR 0009
     option D). Storing ratios makes AC 8's "rescale to match" automatic: a
     width change writes only `stroke_width`. With millimetres, every width
     change would also have to rewrite `stroke_dash`. A width change from
     peer A concurrent with a preset change from peer B would then merge
     into B's millimetres at A's width, which is not proportioned as AC 8
     requires. Inkscape's own dash field is also in multiples of the width.
     AC 9 was reworded on 2026-10-04 to match. Its other guarantee, an
     unlimited list that a later numeric editor reads and writes without a
     format change, still holds.
  5. **A style write that does not change the value writes nothing.** For
     each object, the command compares the new value with the stored value
     exactly and skips that object if they are equal. If no object changed,
     it makes no commit. This is slice 2's "a click must not win against a
     concurrent real edit" rule: an LWW rewrite of an unchanged value is a
     new operation. It also applies to AC 24's multi-object batch.

- **2026-10-04: the gradient stop model.** `fill_stops` is a **Loro movable
  list of stop maps**, the same pattern as a path's anchors, with a
  caller-minted `StopId` per stop. This follows `AnchorId`: it is minted in
  `vecmanf-ui-core` (by the existing minter mechanism, which now serves two
  id types) and is never minted in `document-core`.

  - **Why per-stop registers inside a list** (AC 20): editing one stop's
    colour and a concurrent edit of another stop's position write disjoint
    registers, and both survive. Concurrent add (AC 18) and remove (AC 19)
    are list operations. Selection holds `StopId`s, never list positions
    (ADR 0002 §5's "no array offsets" rule).
  - **Gradient order is a stable sort by position. List order breaks
    ties.** AC 16 says "rendering simply follows whatever stops are
    present, in position order". Every peer renders the same sort, so the
    result is deterministic even after a merge. Coincident stops (AC 18)
    keep the list order the maker created them in, which is what gives a
    hard edge its sides. The panel lists stops in this same order (UX
    notes). Slice 8 exports stops in this order, so SVG's own offset rule
    never comes into play.
  - **Add** (AC 18): the command inserts the stop before the first stop
    whose position is greater, so ties go after existing equal stops.
    **Position edits** (AC 20) write only that stop's `position`. A stop
    dragged past a neighbour changes its place in the sort and is not
    moved in the list. Nothing in this slice issues a `mov`. The movable
    list keeps a later "reorder coincident stops" control free of any
    format change.
  - **2 to 16 is an edit-time rule, not a stored invariant.** The add and
    remove commands refuse beyond those bounds (AC 18, 19). A merge can
    still produce 1 stop (two peers each remove a different stop from 3)
    or 17 or more stops. Open-file validation accepts any count, because a
    legitimately merged document must open. This is AC 16 as reworded.
    Rendering follows SVG: 0 stops means no fill, and 1 stop means a solid
    fill in that stop's colour.
  - **Linear and radial share one stop list.** Switching kind keeps the
    stops. The first switch to a gradient creates the 2-stop list (AC 17)
    in the same commit, from stops the caller supplies with fresh
    `StopId`s, one set per object for a multi-selection. If two peers
    create the list concurrently, Loro keeps one container under the key,
    the same convergence slice 3 relies on for `anchors`.
  - **Rejected: one register holding the whole stop array.** Any two
    concurrent stop edits would clobber each other, which contradicts
    AC 20. **Rejected: an unordered map of stops sorted by position.**
    Coincident stops would then be ordered by id, not by the maker's
    choice, and the hard edge could flip on another peer.
  - **Gradient geometry is in object-bounding-box units** (SVG
    `gradientUnits="objectBoundingBox"`). Resizing a rectangle writes only
    `rect_bounds`, and the gradient follows it without a second register
    that would race. The bounding box is the geometry's (stroke excluded,
    as in SVG), computed in `render-core` at display tolerance. A
    zero-width or zero-height box renders no gradient fill, which is SVG's
    rule. **The UX notes chose a stop editor in the panel only, with no
    gradient redirect, so this slice stores no gradient geometry.** The
    default is fixed here and frozen as the meaning of an absent key, in
    bounding-box units. Linear runs from (0, 0.5) to (1, 0.5), left to
    right across the box, which is Inkscape's default. Radial has its
    centre at (0.5, 0.5) and radius 0.5, so it fits the box and is
    elliptical on a non-square box, as SVG draws it. The follow-up slice
    for on-canvas handles adds one register per kind, each written by one
    handle drag: `fill_linear_axis: [x1, y1, x2, y2]` and
    `fill_radial: [cx, cy, r]`. That needs no migration.

- **2026-10-04: the crate boundary.** The dividing question from slices 2
  and 3 still decides it. Holding style *data* and placing values is
  `document-core`. Turning a curve plus a style into pixels is display
  tessellation, which is `render-core`, at display tolerance. Nothing in
  this slice needs the kernel, and `render-core` cannot reach the kernel
  anyway (ADR 0011 §3).

  - `vecmanf-document-core`: new `style_model.rs` with `Style` (one
    `Stroke` part and one `Fill` part), the `LineJoin`/`LineCap`/`FillKind`
    enums, and the newtypes `Opacity`, `StopPosition`, `DashPattern` and
    `StopId`. New `style_codec.rs` with the keys and absent defaults above;
    `path_codec` and `shape_codec` both call it, replacing
    `read_stroke_width`/`read_stroke`. New `styles.rs` with the command
    methods: one style-field edit over a slice of `NodeId`s (AC 24, one
    commit), the gradient switch carrying per-object initial stops, and
    add, remove and edit for stops. `PathSnapshot` and `PrimitiveSnapshot`
    each carry one `style: Style` field, replacing `stroke_width`/`stroke`/
    `fill`. No trait, because the set of shapes is closed. Open-file
    validation is extended (see the format decision below).
  - `vecmanf-render-core`:
    - `stroke.rs` maps `Style` to `lyon` `StrokeOptions` for width, join
      and cap. The miter limit is a fixed constant of 4 with
      `LineJoin::Miter`, which falls back to bevel as SVG does (AC 11).
      Use `Miter`, not `MiterClip`.
    - New `dash.rs`: dashing with `lyon::algorithms::measure`
      (`PathSampler::split_range` per on-interval), so each dash is a real
      sub-curve that is stroked with its own caps (AC 12). Ratios are
      multiplied by the width here. A pattern that would produce more than
      a fixed number of dashes for one object (on the order of 10⁴)
      renders solid instead. A crafted or extreme pattern must not be able
      to exhaust memory (`project-file-foundation` AC 7, the same reasoning
      as slice 3's point-count cap).
    - New `fill.rs`: `lyon` `FillTessellator` with `FillRule::NonZero`
      (AC 14). Fill sub-paths are closed implicitly, while the stroke uses
      the real open path (AC 15).
    - New `gradient.rs`: stop normalization (stable sort by position, the
      0- and 1-stop rules), the colour ramp for each gradient, and per-vertex
      gradient coordinates in bounding-box space. Interpolation uses
      sRGB-encoded component values (SVG's `color-interpolation`
      default), pinned by a golden ramp test.
    - **One entry point over `&[ObjectSnapshot]` in tree order.** For each
      object it draws the fill and then the stroke. A primitive is drawn
      through `primitive_outline` and the same path builder. This replaces
      today's split, where `build_draw_list` draws all paths and then
      `build_shape_draw_list` draws all primitives. That split was
      invisible with thin black strokes. With fills, a path below a filled
      rectangle would draw on top of it. Decorations stay as they are.
  - `vecmanf-editor-wasm` (`gpu.rs`), as binding plus GPU plumbing:
    - The vertex gains paint data: a solid colour, or a gradient coordinate
      plus a ramp index and a linear/radial flag. The fragment shader
      computes t (linear: the coordinate's x; radial: the length of the
      coordinate) and samples a ramp texture with clamp-to-edge, which is
      SVG's "pad" spread. Everything stays in one vertex buffer and one
      draw call (ADR 0001 §5).
    - **Single coverage per paint layer.** `lyon`'s stroke tessellator
      emits overlapping triangles at joins and self-crossings, and its own
      documentation names translucent SVG strokes as the case where this
      is wrong. Without single coverage, AC 6's alpha double-blends into
      dark spots at every node. Each layer gets its own depth value in
      draw order, with depth test `Less` and depth writes on: one object's
      fill, the same object's stroke, and each decoration. A pixel is then
      written at most once per layer, and later layers still draw over
      earlier ones, with no extra draw calls. **Rejected:** a stencil
      reference per object, which costs one draw call per object, and an
      offscreen layer per translucent object, which costs one render pass
      each.
  - `vecmanf-ui-core`: dispatching a style edit to the current object
    selection (AC 24), the stop-editing state, minting `StopId`s, building
    the default stops for AC 17, the ephemeral preview override that a
    colour-picker or slider drag renders through before it commits on
    release (AC 20's "live"), and the object hit-test order for AC 23 (see
    the hit-testing decision below).
  - `vecmanf-geometry-core`: one new function for AC 23, described
    below.
  - `frontend/`: the Fill & Stroke panel. Binding only.

- **2026-10-04: interior hit-testing (AC 23) is a winding test in
  `vecmanf-geometry-core`.** Whether a point lies inside a curved outline
  means evaluating the curve, which is the slice-2 dividing question
  answered "kernel". `ui-core` already depends on `geometry-core` for
  `nearest_point_on_segment`, so this needs no new edge. The new function
  takes an object's outline anchors (a path's own anchors, or a
  primitive's anchors from `primitive_outline`) and a `Point`. It returns
  whether the nonzero winding number at that point is non-zero. The
  outline is always closed with a straight segment, which matches the fill
  rendering in AC 15. It is built on `kurbo`'s `Shape::winding` for
  `BezPath` (ADR 0003 §2), which is exact on cubics, so it takes no
  `Tolerance`. Points near the edge are already outline hits, at the
  existing hit-test tolerance, before the interior test runs. The module
  and signature belong in `plan.md`.

  `ui-core` decides which objects take part. An object counts as filled
  exactly when `render-core` would paint a fill for it: `fill_enabled`, and
  for a gradient at least one stop. A fill with opacity 0 still counts,
  because it is a non-None fill (AC 23), and Inkscape behaves the same way.
  Hits are resolved topmost first in tree order. The first object whose
  outline (within tolerance) or filled interior contains the point is
  selected. An unfilled object keeps the slice-2/3 outline-only hit area.
  **Rejected: a point-in-polygon test in `ui-core` on its own flattened
  outline.** It would duplicate curve flattening outside the kernel, and
  ADR 0003 §1 forbids that.

- **2026-10-04: interaction commits.** One commit for each of these: one
  committed control change (a colour picker or slider commits on release,
  with an ephemeral preview while dragging); one paint toggle; one dash,
  join or cap choice; one fill-kind switch; one stop add, remove or edit.
  AC 24's multi-object change is one commit for the whole selection. Every
  command resolves all ids before its first write and refuses as a whole
  if any id is stale. A refusal writes nothing.

- **2026-10-04: `format_version` goes to 4.** Migration from version 3 is
  empty by construction: every new key is absent in a version-3 file, and
  absent reads as the frozen default that equals today's rendering. The
  bump is needed for the reader, not the migration. Slice 3's validation
  tolerates unknown keys on purpose. A slice-3 reader would therefore open
  a slice-4 file without complaint and show every fill, dash and gradient
  as a thin black outline. That is the silent partial read ADR 0004 §9
  forbids ("never partially read"); with the bump, that reader refuses the
  file as "newer version". `document.json` adds a `style` object to each
  entry in `objects`, with `StopId`s as hex strings like `AnchorId`s.

  Open-file validation refuses with `OpenError::Damaged`:
  - a present style key with the wrong type or a non-finite number;
  - `stroke_width ≤ 0`;
  - an opacity or stop position outside `[0, 1]`;
  - an unknown join, cap or fill-kind string;
  - an odd-length dash list, a negative dash entry, or a non-empty dash
    list whose sum is 0;
  - a stop missing `id`, `position`, `color` or `opacity`.

  It does **not** refuse on stop count (see the stop model above).

- **2026-10-05: `format_version` is 5, not 4.** `object-transform` is
  inserted before this slice and takes version 4 for its `rotation` field
  (`specs/object-transform/adrs.md`). Everything in the decision above holds
  with "version 3" read as "version 4". If the two slices ship in the other
  order, swap the numbers back.

- **2026-10-05: `format_version` is 6, not 5.** `path-merge-split-and-node-types`
  is inserted before this slice and takes version 5 for its third anchor
  kind (`specs/path-merge-split-and-node-types/adrs.md`). Everything above
  holds with "version 3" read as "version 5". Two consequences for this
  slice's plan: Split writes its new object from the original's
  `PathSnapshot`, so once `PathSnapshot` carries `style: Style`, the new
  piece must get the original's style, and `fill_stops` must be copied
  with fresh `StopId`s, never the original's; Join keeps the surviving
  path's style and discards the other's. Each needs a test.

- **2026-10-05 (architect): the two predecessors swapped order; this
  slice stays at 6.** `path-merge-split-and-node-types` merges first with
  version 4 and `object-transform` follows with 5
  (`specs/0006-path-merge-split-and-node-types/adrs.md`, architect
  resolution). Read "version 3" above as "version 5". A version number in
  an `adrs.md` is provisional: the PR that merges takes `main`'s
  `CURRENT_FORMAT_VERSION + 1`.

- **2026-10-05 (architect): gradient box orientation vs. rotation — open,
  decide once `object-transform` merges.** This slice computes the linear/
  radial gradient box from the object's outline in `vecmanf-render-core`.
  Once `object-transform` ships rotation, that outline is already rotated,
  so the question is whether the gradient box is derived from the object's
  own (oriented) local frame — so the gradient turns with the object, like
  SVG's own `rotate()` transform on a gradient-filled shape — or from the
  rotated outline's axis-aligned bounding box, which would keep the
  gradient's screen-space direction fixed while the object turns under it.
  **Recommendation: the object's own oriented frame** (matches SVG and
  every reference tool's intuition that a gradient is part of the object,
  not the viewport). Not decided now because no rotation exists yet to
  test against; whoever builds this slice after `object-transform` merges
  should confirm this default and add a test, not silently inherit it.

## Flagged to the lead

1. **Resolved 2026-10-04 in `specification.md`.** AC 9 now stores dash
   lengths as multiples of the stroke width, which was the conflict with
   ADR 0009 §3. AC 16 now states 2–16 stops as an edit-time limit and not
   a stored invariant. The new AC 23 makes a filled interior selectable
   (see the hit-testing decision). No acceptance criterion conflicts with
   an ADR.
2. **Two gaps decided here, both consistent with the UX notes.** AC 5:
   entering width 0 means "no stroke", and the last positive width stays
   stored. Rendering: one tree-order pass draws paths and primitives
   together. Today's split draws every path below every primitive, which
   fills would make visible.
3. **For the tester:**
   - AC 6: a translucent stroke over a sharp corner and over a
     self-crossing path. Each pixel must be covered only once.
   - AC 10: a rounded rectangle at effective radius ½ the shorter side. It
     has a zero-length segment, which must produce no spurious join.
   - AC 16: two coincident stops. The hard edge keeps the order the stops
     were created in.
   - AC 23: a zero-opacity fill is still selectable, and a click inside an
     unfilled object selects the filled object beneath it.
4. **For the PO, after this slice:** on-canvas gradient handles (UX notes).
   They add the two geometry registers named above and need no migration.
5. **No new crate, no new dependency** (`lyon::algorithms` is already part
   of the `lyon` facade and `kurbo` is already in `geometry-core`), and no
   ADR amendment.
