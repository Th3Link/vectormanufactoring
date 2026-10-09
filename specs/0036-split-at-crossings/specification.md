# Split at crossings: cut paths into separate pieces wherever they cross

Status: Draft (the criteria are complete and testable; becomes Ready when `adrs.md` and the UX notes exist, `CLAUDE.md` §4)
Priority: Should
Origin: Customer asked for "Split" (request of 2026-10-09) without saying what it splits. **The meaning below is my proposal** (Question 1). If the customer meant Inkscape's "Split Path" (breaking a compound path into pieces that keep their holes), that is `0035-combine-and-break-apart`, Break apart, and this spec can be dropped.

## User value

As a maker I want to select paths that overlap or cross and cut them into separate pieces at every crossing, so that I can delete the part I do not want (an overshooting line, a stub, a hidden overlap that the laser would cut twice), cut a ring open at a chosen place, or turn a drawing of crossing lines into the separate segments a plotter or laser job needs.

**What it is.** The selected paths, open or closed, are cut at every point where one meets another or itself (a crossing, a T-junction where an end lies on another path, a touch). Each path becomes the open pieces between its cut points. Nothing is removed and no piece is merged: the maker deletes the pieces they do not want.

**Reference tools.**

- Inkscape has two commands for cutting, both in the Path menu: Division (Ctrl+/) cuts the *shape* of the bottom path with the top path, and Cut Path (Ctrl+Alt+/) cuts the *outline* of the bottom path with the top path. As I understand them, both take exactly two paths, the stacking order decides which is cut, and the cutting path is used up. The names say little about the difference.
- Illustrator's Scissors and Knife tools cut by hand; its Pathfinder Divide cuts shapes (our `0037-fracture-and-flatten`).
- LightBurn has no cutting of paths; it offers Break Apart on groups only.

Where we do better: one command with a plain name, any number of paths at once, no order and no sacrificed cutter, curves stay curves (the cut is made on the exact Bézier, not on a flattened polygon), and a notice that says how many pieces came out.

## Words used below

- **Operand:** a selected ordinary path (open or closed), or a rectangle, ellipse, polygon or star (its outline as "Object to path" would make it, closed).
- **Contour:** the operand's line to cut: its path, or its outline.
- **Cut point:** a point where two contours meet, or where a contour meets itself: a crossing, a point where an end node or a node of one lies on the other, or a touch. Two contours that share a stretch of line (they overlap along a length) have a cut point at each end of the shared stretch. A cut point is found to within 0.01 mm (the kernel tolerance of `0016`, criterion 25).
- **Piece:** the part of a contour between two consecutive cut points (and, for an open contour, between an end and the nearest cut point).

## Acceptance criteria

1. Given any state of the app, then Split at crossings is a command with a button that follows the rules of `0016` criteria 1, 1a, 2 and 3 (always rendered, dimmed when one object is not selected at least, or the tool is not Select, still focusable, three-line tooltip, no shortcut, never changes the tool). One selected path is enough (it can cross itself). Where it sits is the ux-engineer's decision (`specs/README.md`, rail capacity). Tooltip: "Split at crossings" / "Cuts the selected paths into pieces wherever they cross." / "Replaces the selection. No undo yet."
2. Given operands with at least one cut point, when the maker activates the command, then every operand whose contour is divided (it has a cut point that is not at one of its ends, or, for a closed contour, any cut point) is replaced, at its stacking position and in order, by its pieces as separate open path objects, in the order along the contour. An operand that is not divided is left exactly as it is (same object, same id, same style). All pieces and all untouched operands are selected afterwards.
3. Given a closed contour, then it is cut open: with m cut points (m of 1 or more) it gives m pieces; with one cut point it gives one open piece whose first and last node lie at the cut point. Given an open contour with m cut points that are not at its ends, it gives m + 1 pieces. A cut point at an end node adds no piece.
4. Given a cut point inside a segment, then the segment is divided at that point on the exact curve (de Casteljau) and the two halves keep the curve: every point of both halves lies within 1e-6 mm of the original curve. Given a cut point at an existing node (within 0.001 mm), then that node is split into two end nodes as `0006` criterion 13 does (the first keeps the incoming handle, the second the outgoing one); no node is added.
5. Given a new end node created by a cut, then it is a Corner node and its handle on the open side is zero. Every other node keeps its position, handles and kind. A piece keeps the direction of its contour.
6. Given a piece, then its style is a copy of its operand's style with **Fill set to None** (the stored fill colour and opacity are kept, so Solid restores it). A piece of a closed contour would otherwise paint as an implicitly closed arc, and the cut is about lines.
7. Given a successful command, then exactly one commit is made, stored as `split_at_crossings`, atomic as `0016` criterion 28; pieces have new object and anchor ids; and a notice names the counts: "Split at crossings: 2 paths became 9 pieces. No undo yet." (`role="status"`, 3 seconds, as `0016` criterion 29).
8. Given no operand would be divided (criterion 2), then the command is refused with "The selected paths do not cross each other or themselves. Nothing was changed." (a chip as `0016` criterion 15, no outline) and nothing changes.
9. Given an operand that is a group or a compound path, then it is refused with "Split at crossings does not work on groups. Ungroup first." or "Split at crossings does not work on compound paths. Break apart first. Nothing was changed.", with the offending operands outlined in red as `0016` criterion 15. A primitive is accepted. A coordinate that is not finite or beyond `MAX_COORDINATE_MM` is refused as `0016` criterion 17a.
10. Given the result would have more than 10,000 pieces, then it is refused with "Split would make more than 10000 pieces. Nothing was changed."
11. Given the same objects, then the result is the same however they were selected, and identical on every platform to 1e-6 mm (the same determinism rule as `0016` criterion 43).

### Tests

12. Two open lines (0, 0) to (10, 10) and (0, 10) to (10, 0): four pieces, each from or to (5, 5), within 0.01 mm. A circle of radius 10 mm centred at the origin (nodes at (10, 0), (0, 10), (-10, 0), (0, -10)) and the line (-20, 0) to (20, 0): the line gives three pieces, the circle two arcs, no node is added to the circle (the cuts are at its nodes) and each arc has three nodes. The same circle rotated by 30° (cuts inside segments): two arcs whose points stay within 1e-6 mm of the circle. A figure-eight (one closed path crossing itself once): one open piece. A T: a line ending on the middle of another line: the other line gives two pieces, the first line is left as it is (its end is a cut point at its end node). Two collinear lines (0, 0)-(10, 0) and (5, 0)-(15, 0): cut points at 5 and 10, giving pieces (0..5), (5..10) for the first, (5..10), (10..15) for the second; the doubled stretch (5..10) is not removed.
13. Fixture (golden): a grid of 50 horizontal and 50 vertical lines, 100 paths, 2,500 crossings: the command completes in under 2 s on the reference desktop and gives 5,100 pieces.

## Out of scope

- **Cutting one shape with another and keeping filled pieces** (Inkscape's Division, Illustrator's Pathfinder Divide): that is Fracture in `0037-fracture-and-flatten`.
- **A cutting path that is used up**, and a choice of which path cuts which. All paths cut each other.
- **Removing overlaps** (deleting the doubled stretch of criterion 12) and **removing stubs automatically**: a clean-up command is a good follow-up for laser work, not part of this (Question 3).
- **Compound paths, groups** (criterion 9) and text.
- **Cutting at a point the maker clicks** (a Scissors tool). The Node tool's Split does that at a node (`0006`).
- **Undo and redo** (`0020`).

## Open questions

Each has a default; nothing blocks.

1. **What "Split" means.** *A (default):* this spec. *B:* Inkscape's Split Path, which is Break apart (`0035`), already specified. *C:* a cutting tool or Division. Please say which you meant.
2. **Fill of the pieces (criterion 6).** *A (default):* Fill None. *B:* keep the fill as it is (pieces of a closed shape then paint as closed arcs).
3. **Clean-up of doubled lines.** Default: not in this slice. A "Remove duplicate lines" command (for laser work: a doubled line is cut twice and burns) is a candidate follow-up; say if you want it specified.
4. **Original kept or replaced.** Default: replaced, as the Boolean operations (`0016`). The pieces are the same lines, only cut.
5. **Limit of 10,000 pieces (criterion 10).** Default as written; raise it if you work with larger drawings.

Decided by the product owner (change if you disagree): all paths cut each other; touches count; shared stretches are cut at their ends and kept; pieces are open and fill-less; commit label `split_at_crossings`.

## UX notes

(filled in by ux-engineer before Ready)

## Links

Requirements: R-EDIT-023 (`docs/requirements.md`)
Builds on: `specs/0016-boolean-operations/` (rail command pattern, refusals, kernel tolerance, determinism), `specs/0006-path-merge-split-and-node-types/` (node-level Split)
Related: `specs/0035-combine-and-break-apart/`, `specs/0037-fracture-and-flatten/`, `specs/0023-groups/` (groups refused), `specs/0032-pen-tablet-input/` (eraser, Question 6)
ADRs: `adrs.md` (architect, to come; a curve-curve intersection routine in `curvyo-geometry-core`)
PR: TBD
