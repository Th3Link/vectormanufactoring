# ADRs for "Stroke and fill styling: width/dash/join/cap/color, solid and gradient fill"

This slice extends `path-node-editing` (slice 2) and `primitive-shapes`
(slice 3). It reopens no decision from either. It replaces the placeholder
style both slices store (`stroke_width`, `stroke`, an unstored `fill`) with
one style schema that every object node carries, whatever its `shape`. That
is the document model again, so the schema is written out in full below.

**Read the 2026-10-07 readiness check at the end first: it supersedes the
parts of the notes below that main's merged changes made stale.**

All 25 acceptance criteria can be built against ADR 0002 §5 and ADR 0009 §3
as they read today. **No new crate, no new external dependency, no ADR
amendment.** `curvyo-geometry-core` gains one function, for AC 23's
interior hit-testing. The draft's one conflict (AC 9's dash storage) and
its two open points (AC 16's stop count, interior selection) were resolved
in the specification on 2026-10-04; see "Flagged to the lead".

**Build order:** this slice touches `curvyo-document-core`,
`curvyo-render-core` and `curvyo-ui-core`. `primitive-shapes` and
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
                     new lists are a Loro MERGEABLE child container (marker value in
                     the slot, deterministic container id); a list written earlier is
                     a regular child container; readers accept both (Loro >= 1.16).
                     format_version stays 7. Goldens: styles_v7.curvyo (regular),
                     styles_v7_mergeable_stops.curvyo (mergeable).
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
  `curvyo-ui-core` (by the existing minter mechanism, which now serves two
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

  - `curvyo-document-core`: new `style_model.rs` with `Style` (one
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
  - `curvyo-render-core`:
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
  - `curvyo-editor-wasm` (`gpu.rs`), as binding plus GPU plumbing:
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
  - `curvyo-ui-core`: dispatching a style edit to the current object
    selection (AC 24), the stop-editing state, minting `StopId`s, building
    the default stops for AC 17, the ephemeral preview override that a
    colour-picker or slider drag renders through before it commits on
    release (AC 20's "live"), and the object hit-test order for AC 23 (see
    the hit-testing decision below).
  - `curvyo-geometry-core`: one new function for AC 23, described
    below.
  - `frontend/`: the Fill & Stroke panel. Binding only.

- **2026-10-04: interior hit-testing (AC 23) is a winding test in
  `curvyo-geometry-core`.** Whether a point lies inside a curved outline
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

- **2026-10-07 (architect): ordering note.** Of the pending bumps, this slice
  stays at 6 only if it merges before `rectangle-corner-radii`; the
  number is 7 if `rectangle-corner-radii` merges first (`specs/rectangle-
  corner-radii/adrs.md`, decision 3). The merge rule above decides.

- **2026-10-05 (architect): gradient box orientation vs. rotation — open,
  decide once `object-transform` merges.** This slice computes the linear/
  radial gradient box from the object's outline in `curvyo-render-core`.
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

- **2026-10-06 (architect): gradient box decided; resize-path writes
  re-pointed.** Resolves the note above, after reviewing PR #29.
  - **The gradient box is the object's oriented box**, the same one the
    Select tool draws: `curvyo-ui-core::oriented_bounds` (local-frame
    `min`/`max`, `angle`, `pivot`; geometry only, stroke excluded). A
    rotated object's gradient turns with it, for paths and primitives
    alike. `render-core` does not recompute it: `editor-wasm` passes the
    `OrientedBox` values in the fill draw command (commands carry resolved
    geometry). It has no `ui-core` dependency, so it takes plain points and
    an `Angle`, not the type. SVG export (slice 10): a primitive uses
    `objectBoundingBox` under its `rotate()`; a path, whose anchors are
    baked, uses `userSpaceOnUse` with a `gradientTransform` built from the
    box. Test: rotate a linear-gradient rectangle 90° and the gradient runs
    top to bottom on screen.
  - **Slice 5 writes `stroke_width` directly** in `Document::resize_rect`,
    `resize_ellipse`, `resize_star_frame` and `resize_path`, through
    `path_codec::write_stroke_width`, and `ui-core::select_tool` scales the
    snapshot's `stroke_width` field. *2026-10-06 (architect):* only with
    the Select tool's "Scale stroke width" switch on (slice 5 AC 8, 26).
    The switch is off by default, and then the commands receive
    `stroke_width: None` and leave the key untouched (see
    `specs/0005-object-transform/adrs.md`). When `style: Style` replaces that
    field, those four commands take `Option<Length>` and write a `Some`
    width through the style codec. With the switch on, `select_tool`
    scales `style.stroke_width` only. A resize with the switch off writes
    no style key at all. Dash lengths
    are stored as multiples of the width (AC 9), so they follow the resize
    with no extra write. The `> 0` refusal on open makes the document-core
    resize commands refuse a width that is `≤ 0` or not finite; today only
    `ui-core` floors it (`MIN_STROKE_WIDTH_MM`).

- **2026-10-06 (architect): what `object-transform-refinements` (PR #35)
  adds to the resize path.** Copied from
  `specs/object-transform-refinements/adrs.md`; read the module names above
  as these from now on.
  - The stroke factor is applied in `ui-core::transform_drag::scale_stroke`
    (called by `resize_by_local_delta`), and every resize write goes through
    `ui-core::transform_commit::commit_resize` (via `commit_gesture`).
    `select_tool` no longer touches the width.
  - The typed size entry resolves through the same `resize_by_local_delta`
    and commits through the same `commit_resize`, so re-pointing the four
    resize commands to `style.stroke_width` covers it. There is no second
    call site. Its `StrokeScaling` is read when the chip opens.
  - Skew commits through `resize_path(.., None)` and must keep writing no
    style key. Dash lengths are multiples of the width, so a skew leaves
    them alone.
  - After a skew the oriented box is recomputed tight in the same θ frame
    (refinements criterion 45), so a gradient re-fits to the new rectangle
    and does not shear with the shape. The `0007` tester adds one case: a
    linear-gradient path skewed 30°, gradient box equals the new
    `oriented_bounds`.
  - "Resize commands refuse a width ≤ 0" is never reached by the entry:
    sizes ≤ 0 are refused before resolution and the factor `√(sx·sy)` is
    then > 0.

- **2026-10-07 (implementer): `format_version` is 7.** `rectangle-corner-radii`
  merged first and took 6 (`main` at `bdf4a11`), so this slice takes
  `CURRENT_FORMAT_VERSION + 1` = **7**, by the merge rule above. Read "6" in
  the readiness check, section 1 as 7; the golden fixture is
  `styles_v7.curvyo`, and `rotation_v5.curvyo`, `legacy_corner_radius_v5.curvyo`
  and `corner_radii_per_corner.curvyo` stay as the genuine older containers
  that must open at the defaults.
- **2026-10-07 (implementer): three small decisions in PR 1.** (1) Editing the
  stroke **opacity** also switches an off stroke back on, like the colour and
  the width: the opacity field sits in the colour row of the panel and an
  edit that shows nothing would look broken. Dash, join and cap never switch
  it on. (2) `GradientStop::default_pair` (criterion 17's two starting stops)
  lives in `document-core` beside the types, as a pure function of the stored
  fill colour and two caller-minted ids; `ui-core` only mints the ids (PR 4).
  (3) The four resize commands refuse a width that is not a finite number above
  zero (`ShapeEditError::InvalidStrokeWidth`, `PathEditError::InvalidStrokeWidth`),
  as section 3 of the readiness check requires, before any write.
- **2026-10-08 (implementer): small decisions in PR 3.** (1)
  `StyleEdit::apply_to` is public in `document-core`: `Document::edit_style`
  applies an edit through it, and the panel's ephemeral preview applies the
  same edit to a copy of the snapshots for drawing, so the preview and the
  commit follow one rule (a copy of the rules in `ui-core` would drift, a
  dry-run on a cloned document costs a document per frame). Additive; no model
  or format change. (2) **The panel toggle keeps the view's top-left origin.**
  A viewport resize keeps the view's centre (`canvas-navigation-and-selection`
  criterion 10), which would shift the document by half the panel width. The
  panel announces its width change (`Viewport::keep_origin_for_width_change`);
  the next resize that matches it (that width change, same height, 1.5 px
  tolerance) keeps the top-left, and any other resize is an ordinary window
  resize. Announcements made before one resize add up, so a panel opened and
  closed again leaves nothing pending. Criterion 39 says what must hold, not how.
  (3) **The width field accepts 0 to 1000 mm** (`MAX_STROKE_WIDTH_MM`). No
  criterion sets an upper bound; the stored width is only "finite and above
  zero" (PR 1) and the render guards cap what is drawn (PR 2). 1000 mm is a
  limit of the field, not of the format, and gives the invalid-value message
  something to say. (4) **`ToggleGroup` is built on Radix `RadioGroup`**, which
  moves and selects with the arrow keys, keeps one Tab stop and handles Home and
  End. Radix's own `ToggleGroup` selects only on activation, which is why it was
  not used. (5) **The hex field trims surrounding whitespace** before parsing
  (`" #f80 "` is `#FF8800`); accepted, nothing in criterion 6 forbids it.
  (6) **A gradient mode written through the binding is ignored until PR 4**:
  `Session::set_fill_mode` for linear and radial returns without writing,
  because a gradient without stops is a dead fill (no paint, no clickable
  interior). PR 4 adds the minted seed stops and removes the early return.
- **2026-10-09 (implementer): decisions in PR 4.** (1) **The stop list is a
  mergeable child container** (`LoroMap::ensure_mergeable_movable_list`), which
  answers the review note on concurrent first creation. Two peers that switch the
  same object to a gradient at once now keep both lists' stops and any edits made
  to them, instead of one list replacing the other. The cost: the merged list holds
  both seed pairs (four stops, two at each end, the same colours when both started
  from one stored colour), which criteria 16 and 35 allow and which looks the same.
  Lists that already exist are regular containers and read as before. (2) **The
  ramp function is in `document-core`** (`gradient_ramp.rs`), shared by the
  renderer and the add-stop rule; interpolation is on sRGB-encoded channels and on
  opacity, not premultiplied, over 256 texels. (3) **`Vertex` is not widened**: a
  gradient fill is recorded in `DrawList::gradients` (vertex range, ramp, box) and
  the host derives the per-vertex coordinates. (4) **A stop is addressed by rank**
  in the commands the panel issues, resolved to `(NodeId, StopId)` when a gesture
  starts; a thumb dragged past a neighbour keeps editing the stop it started on.
  (5) **Add stop on an object with no stop list** (a merged or hand-made document)
  seeds the list through the fill-mode switch, since `add_stop` refuses without a
  list. (6) The panel's texts for the stop editor ("No stops. Nothing is
  painted. Add a stop.", the different-counts line, the limit tooltips) are the
  host's, per the UI-text rule; Rust sends counts and flags.

## 2026-10-07 readiness check (architect)

Reference state: `main` at `e285c2d` (slices 5 and 6, `object-transform-
refinements`, `unified-object-editing`, `edit-interaction-polish` PR 1 to 4,
`shape-creation-from-center`, `polygon-star-box-refit`, the Curvyo rename).
Everything below was read in that tree. **Where this section differs from an
earlier note in this file, this section wins.** Crate names are `curvyo-*`.

**Verdict: NOT READY. NEEDS PO CHANGES first (small, listed in 12), then
NEEDS UX (listed in 11). Architecture is buildable. PR 1 and PR 2 (10) can
start as soon as the PO changes land; the UX gaps block PR 3 and PR 4 only.**

### 1. `format_version`

`CURRENT_FORMAT_VERSION` on `main` is 5 (`document.rs`), so this slice
**takes 6**. Still provisional: the PR that merges first takes `main`'s
value + 1 (`rectangle-corner-radii` and `ellipse-arcs-and-shaping`, both
Draft, also want a bump). The bump goes into PR 1, and **PR 1 defines the
complete v6 format** (every key, the stop list and the open-file
validation), so a build from between the PRs never writes a v6 file that
another v6 build misreads. In the PR: change `document.rs`'s doc comment;
`curvyo-editor-wasm/tests/edit_polish_orientation.rs:346` asserts
`CURRENT_FORMAT_VERSION == 5` and must follow; add a `styles_v6.curvyo`
golden fixture (all keys, a 3-stop gradient with coincident stops); keep
`rotation_v5.curvyo` and add the test that it opens with every style
reading its frozen default. `document.json`'s `ObjectJson` (flat
`stroke_width`/`stroke`/`fill` today) gets one `style` object instead.

### 2. Stale or in conflict with merged behaviour

1. **"Build order" paragraph** (starts after `object-transform`): met,
   everything merged. Obsolete.
2. **Version numbers 4, 5, 6 in the 2026-10-04 and 2026-10-05 notes:**
   superseded by section 1.
3. **`build_shape_draw_list`** (crate-boundary note) does not exist.
   Today: `build_draw_list(&[PathSnapshot], view, &DecorationInput)` draws
   path strokes **and** the Node tool's decorations; `build_primitive_strokes`
   draws primitives afterwards; `Session::draw_list` (`session/draw.rs`) glues
   them. The one-pass-in-tree-order decision stands, but the seam is: a new
   `render-core` artwork entry over `&[ObjectSnapshot]` (new module
   `artwork.rs`) and decorations split out of `build_draw_list`. The Node
   tool's live node drag (`live_node_drag_paths_in`) overrides paths, so
   `Session` hands the artwork pass `objects` with those paths substituted.
   `stroke::path_stroke` stays the plain width-and-colour helper that the
   pen preview, the live preview, the guides and `shape_preview` use. Style-aware
   drawing is new code beside it, not a change to it.
4. **`PrimitiveSnapshot` is `Copy`** (`primitive_model.rs`; `primitives_in`
   dereferences it). `Style` carries `Vec<GradientStop>`, so it is `Clone`,
   not `Copy`. **Decision: `PrimitiveSnapshot` drops `Copy`; `Style::clone` of an object with no gradient does
   not allocate (empty `Vec`).** Mechanical `.clone()` fixes at the use
   sites. `Style` derives `Serialize` and `Deserialize` (`PathSnapshot`
   derives both).
5. **Snapshot field replacement touches about 25 test and source files** across the five
   crates (`.stroke_width`, `.stroke`, `.fill`). Mechanical, but part of PR 1.
6. **Hit-testing note, two errors.** (a) It says the outline "is always
   closed with a straight segment". That is right for an **open** path (AC 15)
   and wrong for a **closed** one: a closed path's closing segment is the
   cubic through its own handles (`stroke::build_path`, `segment_pairs`). The
   winding test must close an open path with a chord and a closed path with
   its real closing segment, the same as the fill tessellation. (b) "Hits are
   resolved topmost first, first hit wins" is replaced by the rule in
   section 6.
7. **2026-10-05 note "Split must copy `fill_stops` with fresh `StopId`s,
   never the original's": reversed.** See section 4.
8. **2026-10-05 and 2026-10-06 resize notes:** still right, the call sites are
   now `transform_drag::scale_stroke`, `transform_commit::commit_resize` and
   `numbers_of` (read in section 3). "`select_tool` scales the width" is
   obsolete (the refinements note already says so).
9. **`ui-core::hit_test_object` has three callers**, not one: the press
   (`select_tool/press.rs::classify_press`, which the copy badge also asks),
   the double-click hand-off (`select_tool/entry.rs::double_click`) and the
   hover (`session/select.rs::select_hover`). The 2026-10-04 note names none.
10. **`gradient frame = oriented_bounds` (2026-10-06):** confirmed and
    built (`polygon-star-box-refit` is merged, `oriented_bounds` uses
    `orientation()`). Consequence to state plainly: for a **polygon or star**
    the box is the circumscribed square `C +- (R, R)`, not tight, so a triangle's
    ramp starts at t about 0.25 where the shape begins. **Decision: keep it.**
    The gradient spans the box the maker sees; a tight box is the separate
    story `polygon-star-box-refit` already names. It also means a polygon or
    star turned into a path by "Object to path" gets a tight box, so its gradient
    **re-fits on conversion**. Rectangles and ellipses do not visibly change.
    Known limit, one test, and one line added to the existing
    `docs/technical-debt.md` item on "Object to path" and the shown angle.
11. **Specification, stale or silent** (the PO owns the edits, list in 12).

### 3. What `Style` must do, per merged operation

- **Shape of the type.** `Style { stroke: Stroke, fill: Fill }`;
  `Stroke { enabled, width: Length, color: Color, opacity: Opacity, dash:
  DashPattern, join: LineJoin, cap: LineCap }`; `Fill { enabled, kind:
  FillKind, color: Color, opacity: Opacity, stops: Vec<GradientStop> }`;
  `GradientStop { id: StopId, position: StopPosition, color: Color, opacity:
  Opacity }`. Reuse the existing `Color`. Absent keys read as the frozen
  defaults of the 2026-10-04 note; creation still writes `stroke_width` and
  `stroke` explicitly, nothing else.
- **Resize writes (`resize_rect`, `resize_ellipse`, `resize_star_frame`,
  `resize_path`).** They **already** take `stroke_width: Option<Length>` and
  write only a `Some` that differs from the stored value
  (`shapes.rs::write_stroke_width_if_changed`). Re-pointing is therefore a
  body change, not a signature change: the same key `stroke_width`, now
  through `style_codec`, with the new refusal of a width `<= 0` or not finite.
  `commit_resize` passes `Some(style.stroke.width)` only with "Scale stroke
  width" on (`StrokeScaling::Proportional`, read at press or entry open) and
  `None` otherwise; a resize with the switch off, every skew
  (`resize_path(.., None)`) and every typed size with the switch off write
  **no style key at all**. Dashes are width ratios and follow with no write.
  A stroke that is switched off (`enabled == false`) still has its stored
  width scaled, so turning it on later restores the proportion. Existing tests
  (`resize_with_no_stroke_width_leaves_a_peers_concurrent_stroke_edit_alone`,
  `resize_does_not_rewrite_an_unchanged_stroke_width_or_corner_radius`) stay
  as they are and are the regression net. `transform_drag::scale_stroke`
  and `numbers_of` read `style.stroke.width`.
- **`duplicate_objects`.** It copies the meta map structurally (`copy_map`
  recurses into a nested map and a movable list of maps, no key named), so
  every style key and the whole `fill_stops` list copy **with no change in
  `document-core` and no new field on `CopySource`**. The copied stops keep
  their `StopId`s. **Decision, reversing the 2026-10-05 note: a `StopId` is
  unique within one object's stop list, not across the document.** Every
  command and the panel's selected-stop state address a stop as
  `(NodeId, StopId)`. Fresh ids on copy would need a renumber pass and a
  `CopySource.stop_ids` the caller mints, to protect an invariant nothing
  reads (an `AnchorId` must be global because the node selection holds it
  alone; a stop selection holds the pair). A stop add mints from the same
  minter, so it never collides inside a list. Tests: a copy holds the
  original's style and stops (extend
  `a_copy_holds_every_key_of_the_original_including_an_unknown_one`); editing
  the copy's stop leaves the original untouched and the reverse; both a path
  and a primitive.
- **`split_at_anchor`.** The open-path split writes the new object through
  `create_path_uncommitted(.., stroke_width_mm, stroke, rotation)`, which only
  carries width and colour today. It takes `&Style` instead and writes every
  key through `style_codec::write_style`, `fill_stops` verbatim (same ids,
  section above). Both halves of an open filled path keep their fill and each
  gets its own gradient box (the ramp restarts per half, a known look, no
  stored geometry to carry). The closed-path split keeps the same object and
  changes nothing in its style. Test: split a gradient-filled, dashed,
  round-capped path; both objects read back identical style.
- **`join_endpoints`.** The surviving path `a` keeps every register,
  style included; `b`'s tree node is deleted and its style is discarded, a
  filled `b` joined to an unfilled `a` becomes unfilled. No code change beyond
  the field rename; one test pins it. A same-path join (closing a path) keeps
  the style.
- **"Object to path" (`convert_to_paths`).** It deletes only
  `ALL_PRIMITIVE_KEYS` and writes `closed` and `anchors`; no style key is in
  that list, so the style carries over unchanged. Add a test that no style
  key is ever added to `ALL_PRIMITIVE_KEYS` and that a converted gradient path
  keeps its stops. Look change: section 2, item 10.
- **`rotate_object`, `translate_objects`.** Untouched. A rotated object's
  gradient turns with it (the box turns, no key written).
- **The live preview, two different previews, two answers.**
  (a) The Select tool's geometry preview (resize, rotate, skew, move, bar
  edits; `build_live_edit_preview`) stays what `unified-object-editing`
  criteria 10 to 15 say: a hollow 1.5 px `--preview-new` outline of the new
  geometry over the committed object in its own committed style, no fill and
  no stroke-width preview (the criterion's own note: "no fill preview yet,
  revisit with `0007`"). **Decision: revisit = keep.** The gradient box and the
  stroke scale are recomputed from the final geometry on commit; drawing
  them live would draw the object itself, which is the opposite of
  blue-new, black-old. The PO confirms (12); no code change.
  (b) The panel's slider, picker and stop drags are a different preview: the
  object itself, drawn in the new style. It is ephemeral, never in the
  document (ADR 0009 section 2): `ui-core` holds a `StyleOverride`
  (`NodeId` set plus the one pending `StyleEdit`) that `Session::objects` applies
  to the snapshots before the artwork pass, the way `live_node_drag_paths_in`
  substitutes a drag. One commit on release, per the specification. It cannot
  coexist with a Select drag (one pointer).
- **Selection box, hover box, blue outline over filled artwork.** All three are
  drawn after the artwork, so they are on top; no render-order change. Their
  **colour** is a UX gap (section 11): a 1 px `--accent` dashed box on a blue
  fill, and the 20 % `--accent-hover` hover box on any dark fill, are
  invisible, and every measurement in `design-system.md` was taken on the
  canvas colour only.

### 4. Gradient stops across multi-selection (a command-shape correction)

The specification shows the stop list for a multi-selection only when all
objects have the same fill mode and stop count (UX notes). The objects have
different `StopId`s, so a stop edit in that case cannot carry one id. **The
command is per object:** `ui-core` maps the stop at rank `k` (position order,
list order breaks ties, as in the stop-model note) to each object's own
`StopId` from the snapshot it just read and hands `document-core` a list of
`(NodeId, StopId, change)`. A stale `NodeId` or `StopId` refuses the whole
batch (the existing rule). `document-core` still never addresses a stop by
rank or list position.

### 5. Rendering decisions that the earlier notes left open

- **Display floor and dashes.** `MIN_DISPLAY_STROKE_WIDTH_PX` still floors the
  drawn width (`min_display_stroke_at_extreme_zoom_out.rs` pins it). Dash
  lengths come from the **document** width times the ratio, not the floored
  width. A pattern whose period (on + off) is under 2 screen px, or one that
  would produce more than the per-object dash cap, renders **solid**. The cap
  of 10^4 per object is too high for a per-frame, uncached tessellation of
  hundreds of objects: **use 2000 per object** and a fixed per-frame total
  (50 000), solid beyond either. Preset dashes need `on > 0` so that they show
  under butt caps (a zero-length "Dot" is invisible); the preset numbers are
  a UX item.
- **Single coverage via depth.** The decision stands but has three costs
  the 2026-10-04 note did not state. (1) It needs a depth attachment with the
  **same MSAA sample count** as the colour target (`gpu.rs`
  `depth_stencil: None` today): the memory in the existing debt item
  "MSAA x HiDPI memory" about doubles. Measure in PR 2 on the customer's
  machine; the fallback lever is the existing `PREFERRED_SAMPLE_COUNTS`.
  (2) `gpu.rs` compiles only for wasm32 and is 695 lines, so the layering,
  the ramp texture and the shader cannot be tested natively. Everything that
  can be pure is: the layer assignment and the ramp/colour-at-t function in
  `render-core` with native tests; for the GPU, a recorded browser
  pixel-read check (a translucent self-crossing stroke has no dark spots at
  nodes). (3) `Vertex` has 9 construction sites, so widening it is small,
  **but it widens only in PR 4**, when a gradient needs it; PR 2 adds depth
  layers and keeps colour-only vertices.
- **Cost of reading.** A frame at rest reads every object (about 12 ms of
  the measured 18 ms for 200 objects, `docs/technical-debt.md`, canvas
  performance). Each object now reads about 13 more keys, and fills and
  dashes are tessellated every frame. **PR 1 re-runs the `#[ignore]`
  benchmark with styled objects; if a 200-object frame at rest passes 25 ms,
  the draw-list cache keyed by document version (that debt item's
  resolution) moves into PR 2.** Otherwise it stays deferred.

### 6. Hit-testing and the press order (AC 23)

`hit_test_object` is the one ordering function (all three callers use it),
`advanced-selection`'s `hit_test_objects` does not exist yet, and `0007` ships
first, so this section fixes the order and `advanced-selection` follows.

- **Interior test.** `curvyo-geometry-core` gets one function (`kurbo`
  `BezPath::winding`, exact on cubics, no `Tolerance`): the anchors, a
  `closed` flag, a point. Closed: the closing segment is the real cubic.
  Open: a straight chord. `ui-core` rejects by `object_bounds` first, so a
  hover over thousands of objects does not run the winding test on every
  one. An object takes part exactly when `render-core` would paint a fill
  (`fill_enabled` and, for a gradient, at least one stop); opacity 0 counts.
- **Rule for one point.** Let F be the topmost object whose filled interior
  contains the point. Among the objects at or above F (all objects if there
  is no F), the nearest outline within tolerance wins, an exact tie going to
  the topmost (the slice 4 rule, **unchanged when nothing is filled**). If no
  outline is hit, F wins. An object below F cannot win: F covers it. This
  keeps slice 4's nearest-outline behaviour and stops a hidden outline under
  an opaque fill from beating the fill. For `advanced-selection`: the
  cycle list starts with this result and continues with the remaining
  outline hits nearest first, then the remaining interior-only hits topmost
  first. Its flag 2 is resolved by this.
- **Press order, as it stands and how fill changes it.** Today
  (`classify_press`): handle, centre handle, then (no Shift) inside the sole
  selected object's box = move, then `hit_test_object`, then empty = marquee
  or deselect. This slice changes only the fourth step: it now returns an
  object whose outline **or filled interior** is under the point.
  Consequences, all intended: a drag that starts inside an unselected filled
  shape moves it (it no longer starts a marquee); a Shift press on any
  filled interior toggles that object (this is what criterion 35 means by
  "add-to-selection works over a filled shape"); a Shift press inside the
  sole selected box away from every object still finds nothing. **Criterion
  35's text must say "outline or filled interior" in its last two clauses
  (PO edit, 12).**
- **Hover and double-click.** Hover (`select_hover`) and the double-click
  hand-off (`double_click`) call the same function, so a filled interior
  lights up the hover box and a double-click on a filled path's interior
  hands it off to the Node tool. Hover should also follow `classify_press`
  (inside the sole selected box lights nothing else, because a press there
  moves the selected object): today it does not, and fills make the
  mismatch visible. A small fix inside this slice, with a test.
- **One customer question (CLAUDE.md section 3: it changes accepted
  behaviour, so ask).** With fills, "inside the sole selected box moves"
  (`unified-object-editing` 35, slice 5 criterion 23) has a trap: select a
  large filled rectangle, and a click on a smaller filled shape lying on it
  moves the rectangle instead of selecting the shape. Option A: keep the
  order (the only way to the shape is deselecting first, or Alt-click once
  `advanced-selection` ships). Option B: the move yields to a **different
  object above the selected one whose filled interior contains the point**
  (outlines of other objects still do not take the press, as slice 5 says).
  **Recommendation B; default if unanswered: A** (no accepted behaviour
  changes, and B is a one-condition change in `classify_press` later).

### 7. Properties panel against the floating Select bar

- No conflict of purpose: the bar is the Select tool's **permanent** bar
  (customer, 2026-10-06; the switches, Radius, Points, Ratio, Object to
  path); the panel is the docked, tool-independent Style section. Neither
  absorbs the other in this slice.
- **Layout conflict to fix.** `App.tsx` anchors the bars' overlay row at the
  window edge (`absolute right-3 left-[72px]`). With a 280 px docked panel the
  bar would run underneath it. The overlay moves into the canvas region (a
  `relative` wrapper around `Canvas`) and keeps `left-[72px]`; the bar wraps
  within the narrower canvas (it already wraps). The rail stays floating
  over the canvas. Collapsing the panel resizes the canvas through the
  existing `ResizeObserver` path, which slice 4 already requires to be stable.
- `design-system.md` (2026-10-05) says the shape tool's persistent options
  (point count, ratio) move into the panel as a "Shape tool options" section.
  `unified-object-editing` made the shape tools creation-only and the
  polygon/star tool keeps its floating bar. **This slice builds the panel with
  the Style section only.** UX corrects that paragraph.
- **Selection scope per tool is unspecified.** The panel edits the object
  selection (`Session.selection`), which the Select tool and the shape tools
  share. Under the Node tool the nodes of a path are selected, not the
  object; under the Pen tool nothing is. PO/UX state it. Recommendation: the
  Node tool edits the paths owning the selected nodes, the Pen tool and an
  empty selection show the disabled state. "Last-used value" for a disabled
  panel is dropped (nothing to edit, so show the frozen defaults).
- Focus: the other bars return focus to the canvas after an action
  (`onReturnFocus`). A form that is tabbed through should not. UX decides
  per control. Letter shortcuts are already ignored while a control has focus
  (design-system, "Any letter shortcut"), so typing in a field cannot trigger
  a tool. Test: Backspace and Delete in the width field and the hex field do
  not delete the selected object.
- **Frontend gaps.** `components/ui/` holds only `alert-dialog` and `button`.
  The panel needs Popover, Select, ToggleGroup, Slider and Tooltip wrappers
  (the `radix-ui` umbrella package is already a dependency; `shadcn`'s CLI
  is the one the debt item "`shadcn` pulls a vulnerable `braces`" is about) and
  a colour area. Default: the wrappers by hand over `radix-ui`, the
  saturation/hue area from `react-colorful` (MIT, no dependencies, keyboard
  support), the lead justifies it in the PR. The panel's state goes in a new
  `useStylePanel.ts`: `useEditorSession.ts` is 1409 lines and does not grow.

### 8. Module size debts: does this slice need to split first?

**No pre-split is required.** Non-test sizes measured: `select_tool.rs` 379,
`session/mod.rs` 499, `wasm_api.rs` 743 (the only file over the limit among
the three), `gpu.rs` 695. The new code goes into new modules:
- `document-core`: `style_model.rs`, `style_codec.rs`, `styles.rs`. The old
  `read_stroke*`/`write_stroke*` leave `path_codec.rs` (524, shrinks).
- `geometry-core`: the winding function beside `nearest_point_on_segment`.
- `ui-core`: `style_panel.rs` (the state, reusing `BarValue` for mixed),
  `style_edit.rs` (the override and the dispatch). `hit_test_object.rs` and
  `select_tool/press.rs` change in place (274 and 133 lines).
- `render-core`: `artwork.rs`, `dash.rs`, `fill.rs`, later `gradient.rs`.
- `editor-wasm`: **`wasm_style.rs`** (the pattern of `wasm_select_bar.rs`), so
  `wasm_api.rs` does not grow; `session/style.rs`. `session/mod.rs` is at
  499: a new `Session` field pushes it over, so PR 3 task 1 is a pure move of
  the tolerance helpers (about 40 lines) to `session/tolerances.rs`.
- `gpu.rs` (695, over the limit): **PR 2 task 1 is a pure move of the
  pipeline and shader set-up into `gpu_pipeline.rs`**; the depth attachment
  and (PR 4) the ramp texture go there and in `gpu_paint.rs`, not into
  `gpu.rs`. A dated line is added to the `Session` size item in
  `docs/technical-debt.md`.
`wasm_api.rs` stays with `advanced-selection` as the debt item says.

### 9. What is not buildable as written

1. The hit-test order "topmost first, first wins" (occlusion and slice 4's
   nearest rule): replaced, section 6.
2. "Closed with a straight segment" for closed paths: replaced, 2.6.
3. A stop addressed by `StopId` alone in a multi-selection: replaced, 4.
4. Fresh `StopId` on split and copy: reversed, 3.
5. `Copy` on `PrimitiveSnapshot` with a stop list inside: dropped, 2.4.
6. The 10^4 dash cap for per-frame tessellation, and dashes at the display
   floor: replaced, 5.
7. AC 21, 22: "bounding box" is the oriented box (the selection box); stated in
   the amendment in 12.
8. The panel at the window edge under the bars, and the shape-options
   section: section 7.
Nothing here needs a new crate, trait, generic or dependency beyond
`react-colorful` (frontend, justified in the PR). **No ADR amendment, nothing
`needs-customer` except the one question in section 6.**

### 10. PR split (4 PRs, stacked; PR 1 alone is invisible)

1. **Model, no visible change.** `document-core` (`Style`, codec, all keys
   and the stop list, validation, the stop commands, `format_version` 6,
   fixtures, `document.json`), the snapshot replacement and the 25 test
   files, the resize/duplicate/split/join/convert tests, `ui-core`
   re-pointing (`scale_stroke`, `commit_resize`, `numbers_of`), and
   `render-core` reading `style.stroke.width` and `.color` only (so AC 3
   holds). Benchmark re-run. AC 2 to 6, 9, 13, 16 to 20 (storage and
   commands), 24, 25.
2. **Rendering and hit-testing.** Task 1: `gpu.rs` pure move. The artwork
   pass in tree order, stroke on/off, alpha, dash, join, cap, solid fill with
   the closing chord, depth layers, the winding function, the new
   `hit_test_object` rule, press, hover and double-click, the box and
   outline contrast the UX note fixes. AC 6 to 8, 10 to 15, 23. Tested by
   draw-list and native tests plus the recorded browser pixel check.
3. **Panel (the first demo).** Task 1: `session/tolerances.rs` pure move.
   `PropertiesPanel` with the Style section (stroke and solid fill),
   `wasm_style.rs`, `style_panel.rs`, mixed state, the override preview,
   colour-alpha picker, `Shift+Ctrl+F`, `App.tsx` re-anchoring,
   `design-system.md` rows. AC 1, 2, 4 to 9, 13, 14, 24 through the UI.
4. **Gradient.** `Vertex` widening, ramp texture, shader (`gpu_paint.rs`),
   `gradient.rs` (stop sort, 0 and 1 stop rules, colour at t, sRGB-encoded
   interpolation, pinned by a golden ramp), `editor-wasm` passing the
   `OrientedBox` values in the fill command, the stop editor, linear and
   radial. AC 16 to 22. Tester cases: rotated 90 degrees, skewed 30 degrees
   path, "Object to path" re-fit of a polygon, coincident stops, zero-size box.
If the lead wants three PRs, merge 1 and 2; do not merge 3 and 4.

### 11. What the UX engineer must specify before Ready (missing from the specification)

1. **Default colours of a new gradient and of an added stop.** AC 17 and the
   UX notes each point at the other; neither says. Also the "Add stop"
   position rule ("1.0 minus a small offset": which offset) against the
   position-order insert in this file.
2. **Dash presets**: the on/off ratios of Dash, Dot, Dash-Dot (`on > 0`,
   section 5) and the line-sample icons.
3. **Contrast of the dashed selection box, the hover box, the blue preview
   outline and the handles over filled artwork** (section 3). One proposal to
   test: a 1 px white under-line below the `--accent` line, or a mixed-blend
   line. No measurement exists on anything but the canvas colour.
4. **Panel layout** inside 280 px: rows, label column, control heights (the bars
   use 28 px), section header, the checkerboard and "Mixed" tokens with
   their contrast, the `ColorAlphaPicker` popover placement and size inside a
   docked panel, alpha as an integer percent (stored as a fraction; rounding
   rule for "reads back as set").
5. **Gradient stop list**: multi-selection by rank (section 4); display when
   a merged document holds 0, 1 or more than 16 stops (add disabled at 16,
   remove disabled at 2 or fewer, a list of 0 or 1 still shows and edits);
   selected-stop state.
6. **Selection scope under the Node and Pen tools**, focus return after a
   commit, and the panel in a narrow window.
7. **Correct `design-system.md`** (shape options section, the permanent bar
   and panel together, the overlay anchor).

### 12. What the PO must change in `specification.md` (small, blocking)

1. **New criterion, draw order:** objects draw in tree order, paths and
   primitives together. This is a visible change for existing files (a path
   above a rectangle drew *under* it until now).
2. **New criterion, hit order with fills:** the press order of section 6,
   the hover and double-click on a filled interior, and the customer's answer
   to the section 6 question. Reword `unified-object-editing` 35 (last two
   clauses) and `advanced-selection` AC 3 to 5 and flag 2 to match.
3. **New criteria for duplicate, split, join and "Object to path":** the
   style is kept as section 3 says (copy keeps style and stops, Split copies
   it to the new object, Join keeps the survivor's, conversion keeps it).
4. **AC 21 and 22:** "the object's own bounding box" becomes "the object's
   selection box (the oriented box the Select tool draws)", turning with a
   rotation, re-fitting after a skew; add the polygon/star look and the
   "Object to path" re-fit as stated limits.
5. **AC 16** (a stop is edited in a multi-selection by rank) and the
   0/1/more-than-16 stop display rule go with the UX note, 11.5.
6. **Confirm** that a geometry drag keeps showing no fill or stroke
   preview (section 3), and that the shape tools keep their floating bar.
7. **Statuses:** `0005`, `0006`, `edit-interaction-polish`, `unified-object-
   editing` and others read "Ready" but are merged; the PO sets them to Done
   with the PR links (the index in `specs/index.md` is the PO's).

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
