# Dashed selection box: UX notes (Part E experiment)

Author: ux-engineer. Companion to the screenshots `dashed-V<n>-<scene>.png` and
`dashed-comparison.png` in this folder. Nothing here is a decision; the customer decides.

## Recommendation

Keep the solid box as the default unless the customer actively prefers the dashed look. If
he does, take **V2 (6 on / 3 off)**, not V1 or V4. Reasons:

1. **V1 and V4 use the 4 / 3 pattern of the lasso and of the skew fixed-line guide.** During a
   skew drag the guide runs along the box's own edge (`docs/design-system.md`, skew fixed-line
   guide). With a 4 / 3 box the guide and the box edge become the same pattern on top of each
   other, and only the 16 px extensions past the box ends stay readable. A box in a different
   pattern keeps the guide distinguishable.
2. **V3 (2 / 2) is too fine.** On rotated edges and at 1x it dissolves into a grey-blue mid-tone
   (see `dashed-V3-rotated.png`); it reads as noise, not as a box.
3. **V2 has the longest dashes**, so the line stays legible when anti-aliasing eats one pixel at
   each dash end, and it is the least busy against the object's own outline.

Whatever pattern is chosen, two changes matter more than the dash length (below): snap the box
line to the pixel grid, and centre the dash phase on each edge so corners close.

## What the screenshots show

Method: throwaway build (`ux/dashed-box-experiment`), the dash pattern switchable at run time,
the same document for every variant, canvas pixels captured directly (no toolbars). 1x is the
pane's own density. The "2x" files come from forcing `devicePixelRatio` to 2 and re-sizing the
backing buffer; it is not a real 2x display. "zoom200" is the view at 197 percent. Only one
theme exists in the app (canvas `#E8E8EB`); there is no dark theme to capture. Shapes are
stroked, unfilled (the only style the app has), so the box coincides with the object's own
1 px dark outline for rectangles.

**Contrast on the canvas (measured, WCAG ratio against `#E8E8EB`):**
- `--accent` at full coverage: 3.7:1. Passes 3:1.
- The box as drawn today at 1x, when its edge falls between two pixel rows: it renders as two
  rows at about 50 percent (`#8BABEC`), 1.9:1. Fails 3:1. Whether a given edge is crisp or
  smeared depends on where the object sits and on the zoom; at 2x the measured edge was two crisp
  device rows at full `--accent`. This is already true for V0 today and is independent of dashes.
- `--accent-hover` (20 percent) is 1.1:1: the hovered box is barely visible in every variant,
  solid or dashed. Dashing it makes no visible difference (`dashed-V<n>-multi.png` vs
  `-multi-hoversolid.png`), so "selected vs hovered stay distinct" holds, but only because the
  hover box is faint in all cases.

**Against the handles:** handles are 14 px squares drawn on top of the corners and edge midpoints
and stay fully readable in all variants. Dashes under or next to a handle do not compete with it.
With one object selected, the handles hide the corners, so phase problems there are masked. They
show in a multi-selection, where there are no handles (`dashed-V1-multi.png`).

**Dash phase and corners.** The dashes restart at each edge's first corner, in the box's own
frame (continuous with the rotation, no dependence on pan, zoom or time, so nothing crawls).
The far end of each edge then ends at an arbitrary point of the pattern: some corners close,
some end in a gap (visible in `dashed-V1-multi.png`, bottom right of the lower box). V4 closes
all four corners by construction. For V1, V2 and V3 a pattern centred on each edge, with a
dash at both ends, would give four closed corners and a symmetric look. That needs one more
line of logic than "start at corner 0"; I recommend it if any of V1 to V3 is adopted.

**Dashes over the object's outline.** Where the box lies on a dark outline (rectangles, and any
object whose bounds touch its outline), each gap shows the black stroke, so the line alternates
blue and black. It reads as a blue-black dashed line, not as an accent-coloured one, and for
fine patterns it looks like grey with a blue tint. The solid box instead overpaints the outline.
This is the strongest visual argument for solid, and it gets worse the finer the pattern.

**Zoom and flicker.** Dash lengths are screen pixels, so 100 percent and 200 percent look the
same (`-rect-zoom200.png`); the only change is where the pattern starts relative to the pixel
grid. Static captures show no moire for V1, V2, V4. V3 shows the beginnings of it on rotated
edges (stair-stepping of 2 px dashes on a 30 degree edge). I could not check motion in a still
capture; by construction the phase is a pure function of the box corner, so panning moves the
pattern rigidly with the box.

**Rotated box (30 degrees).** All dashes follow the rotated edge. Diagonal dashes are
anti-aliased, so a 4 / 3 or 6 / 3 pattern loses about a pixel of contrast per dash end; V2 copes
best, V3 worst.

**Tiny object (about 20 px).** The box is a 20 px square between four 14 px handles, so only
about 6 px of each edge is visible. V1 to V3 show one or two dashes per edge, with no pattern
at all. V4 shows ticks that nearly fill each edge, so it looks solid on some edges and has one short dash on the others (no worse than V0 there).
At that size the dash adds nothing; it only subtracts contrast.

**Confusion with the lasso.** The lasso is on screen only during an Alt-drag, so confusion is
unlikely, as the spec guessed. The real overlap is the skew guide, covered under point 1 above.

## Findings outside the dash question

1. Snap the selection box line to device pixels (or draw it at a pixel centre): it takes
   the line from 1.9:1 to 3.7:1 at 1x wherever the object sits, and it helps every variant.
   This is an issue today, unrelated to dashes.
2. `--accent-hover` at 20 percent (1.1:1) is below any legibility threshold; it works only
   because the hover box is a hint. If the customer cares about hover feedback, a stronger
   token is a separate decision.

## Files

`dashed-V<n>-<scene>.png`, n = 0 to 4. Scenes: `rect` (120 px rounded rectangle, radius
10, all handles), `rotated` (about 30 degrees), `multi` (three selected, a fourth hovered, a large
rectangle crossing them; `-hoversolid` shows the hover box solid), `tiny` (about 20 px).
Suffixes: none = 1x, `-2x`, `-zoom200` (rect and rotated only). Crops are magnified with
nearest-neighbour (rect 3x, rotated 3x, multi 2x, tiny 6x at 1x; the 2x files are
scaled to a comparable size). `dashed-comparison.png`: all five variants, four scenes at 1x.
