# Pen path extension: continue an open path, connect two paths, choose how a path closes, close a selected path

Status: Ready (`adrs.md` and the UX notes exist, both 2026-10-10)
Priority: Should
Origin: Customer (the four requests: extend an open path from either end, connect two paths with the Pen, choose a sharp or smooth closing node, an auto-close helper). The modifier, the defaults, the preview, the labels and the "Shift means the alternative" rule are my proposals, marked in "Decided by the product owner" and in the open questions.

## User value

As a maker I want to pick up an unfinished line where I left it, join two lines by drawing from one end onto the other, and close a path either with a sharp corner or with a smooth curve, so that I never redraw a path or leave a gap or a kink at the point where it closes.

**What exists today (checked in the code and `0002`).** The Pen always starts a new path. Clicking the first node of the path in progress, with two or more nodes placed, closes it; the pointer within 16 px of that node shows a close cursor and a hover ring (`0002` UX notes). Closing adds a segment from the last node to the first and leaves the first node as it was drawn, so a path drawn with clicks closes with a corner and one drawn with drags closes smooth, and the maker cannot choose. A finished open path can only be extended in the Node tool by Join (`0006`), which moves two nodes into one at their midpoint.

**Reference tools.** Inkscape's Pen continues the selected path from an end anchor and joins to another path's end anchor; closing by clicking the first node, and a drag there shapes the closing handle. Illustrator's Pen shows a cursor change over an end point (continue) and over another path's end point (join), and a drag on the closing point makes it smooth. Where we do better:

- Every state is shown before the click: a ring, a cursor and a hint chip name what the click will do (continue, join, close sharp, close smooth), and the closing segment is previewed as it will be drawn.
- One consistent key: **Shift always means "the alternative at this target"**, shown in the chip. Over an end node it starts a new path instead of continuing; over another path's end it places a plain node instead of joining; over the close target it flips sharp and smooth.
- Escape on a continuation throws away only what was added; the path that was there before is never touched until the maker finishes. This matters because there is no undo yet (`0020-undo-redo`).
- A "Close path" command in the Node bar closes an existing open path, with the choice written on the buttons.
- The connect rule is written down: which path keeps its style and its place, and in which order the nodes end up.

## Words used below

- **End node:** the first or the last node of an open, ordinary path (one outline). Closed paths, compound paths (`0016`) and primitives have none.
- **Continuing:** the Pen state after the maker pressed an end node E of an existing path P. P is the **continued path**; it stays in the document unchanged and is drawn as it is.
- **Target:** a node the pointer is within **16 px** of (the node hit radius, `docs/design-system.md`, the same radius as the existing close target). Nearest wins; a tie goes to the topmost object, then to the last node.
- **Closing node:** the node the closing segment arrives at, in the direction the maker draws (or, for the Close path command, the path's first node). It is the node the maker clicks to close.
- **Join type:** **Sharp** means the closing node is a Corner node (`0006`: independent handles, a cusp). **Smooth** means it is tangent-continuous: Symmetric or Asymmetric (`0006`). The terms of `0006` are used; "Smooth" is not a node kind.

## Acceptance criteria

Distances are document millimetres unless a criterion says "px" (screen pixels). Node order is the stored order; direction of drawing is the order the maker placed nodes.

### Part A: continue an open path

1. Given the Pen tool with no unfinished path, when the pointer is within 16 px of an end node of an open path (and not on the active selection's handles or another tool's overlay), then, at once, that node is drawn with its idle node glyph (14 px, the shape of its kind) inside the hover ring (`--accent-hover`, 18 px, as in the Node tool), and the cursor is the Pen "continue" variant. After 600 ms of rest on the target the two-line hint chip appears: "Continue path" / "Shift: start a new path". Nothing shows over interior nodes, closed paths, compound paths or primitives. With Shift held the ring is not drawn and the cursor is the plain Pen nib, and the chip reads "Start a new path" / "Release Shift: continue path"; the chip appears at once when Shift is pressed or released over the target. The cue follows Shift live, also with the pointer at rest.
2. Given the cue of criterion 1 without Shift, when the maker presses and releases on the end node E (a drag is ignored, as for the close target), then the Pen is Continuing: P is unchanged in the document; E is drawn as the most recently placed node (permanent hover ring, `0002` UX notes); the rubber band runs from E; the Properties panel is empty (`0015` criterion 14a, an unfinished path). With Shift held at the press, a new path starts at that point as today.
3. Given Continuing, then nodes are placed by click or click-drag exactly as `0002` criteria 1 and 2 define, and the first new segment starts at E using E's handle on its open side as stored (E's outgoing handle if E is P's last node, its incoming handle if E is P's first), possibly zero.
4. Given Continuing from P's last node, then the new nodes are appended in the order drawn. Given Continuing from P's first node, then the new nodes are placed before P's first node, so the order is [last new node, ..., first new node, P's nodes]. P's own node order never changes, and each new node's handles are assigned so the curve is as drawn. Test: P has nodes (0, 0), (10, 0), (20, 0). Continuing from (20, 0) with clicks at (30, 0) and (40, 0) gives (0, 0), (10, 0), (20, 0), (30, 0), (40, 0). Continuing from (0, 0) with clicks at (0, 10) and (0, 20) gives (0, 20), (0, 10), (0, 0), (10, 0), (20, 0).
5. Given Continuing with at least one new node, when the maker finishes (Enter or the finishing double-click of `0002` criterion 3), then exactly one commit is made, stored as `extend_path`: P keeps its object id, its style, its place in the stacking order, its selection state and the ids of its existing nodes; the new nodes get new ids; P's node list is replaced by the longer one. It is atomic (`0015` criterion 19). Given no node was added, then finishing writes nothing.
6. Given an end node of an open path P, then criteria 1 to 5 apply whether or not P is selected. (Inkscape continues only the selected path; see Question 3.)

### Part B: connect two paths

7. Given Continuing, or a new path in progress, when the pointer is within 16 px of an end node Qe of another open path Q (a different object; the in-progress path's own close target wins when both are in range), then Qe is drawn with its node glyph inside the hover ring, the cursor is the Pen "join" variant, the rubber band ends on the node's centre, and the two-line chip appears after 600 ms of rest on the target: "Join with path" / "Shift: place a node". When the maker is continuing a path P and the style of P differs from the style of Q, the chip has a third, muted line: "The result keeps the style of the path you continue." (a new path that ends on Q becomes Q, so it has no third line). With Shift held, the ring is not drawn, the cursor is the plain nib, the chip reads "Place a node" / "Release Shift: join with path" (at once when Shift changes over the target), and a click places a new node there, as today.
8. Given the cue of criterion 7 without Shift, when the maker presses and releases on Qe, then the in-progress path and Q become one path, finished at once, by this rule. The **surviving object** is the continued path P (its id, style and place); if the in-progress path is a new one, it is Q. The other object is removed. The node order is: the surviving object's nodes in their order; then, at its open end that was joined, the other object's nodes in the order that makes the joined ends adjacent, reversed (with each reversed node's in and out handles swapped) if needed, so the surviving object's order is never changed (the rule of `0006` criterion 9 for the first path). Test: P has (0, 0), (10, 0); Q has (30, 0), (40, 0); continue P from (10, 0) and press Qe = (30, 0): the result is (0, 0), (10, 0), (30, 0), (40, 0). Press Qe = (40, 0) instead: (0, 0), (10, 0), (40, 0), (30, 0). Q is gone.
9. Given criterion 8, then the last placed node (or E, when none was placed) and Qe are joined by one segment drawn from the handles those two nodes have; their kinds and handles are unchanged. Given the two nodes are within 0.001 mm of each other, then they are merged into one node by `0006` criterion 9 (midpoint, each side's handle kept, Corner) instead of leaving a zero-length segment.
10. Given criterion 8, then the surviving path's style is kept and the other object's style is discarded. Test: P red 2 mm, Q blue 0.5 mm: the result is red 2 mm. A new path being drawn has the placeholder style of `0002` criterion 6, so a new path that ends on Q becomes Q.
11. Given the finish by joining, then exactly one commit is made, stored as `connect_paths`, that writes the longer path and removes the other object together, atomically: no state exists in which both objects or neither exist.
12. Given Q is a compound path, a primitive or a closed path, then it has no end node and no cue appears. Given Qe belongs to P itself (the other end of the continued path), then it is the close target of Part C, not a connect target.

### Part C: closing, and how the closing node joins

13. Given the in-progress path (new or continued) would have at least 3 nodes when closed, when the pointer is within 16 px of the closing target, then the existing cue shows (close cursor, hover ring on the node) and the chip names the join (criterion 16). For a new path the target is its first node; for a continued path it is P's other end node F. A continued path with only P's two nodes and no new node has no close target (a closed path of two nodes is refused as in `0006` criterion 8).
14. Given a close press, then it closes on release, exactly as today (a drag is ignored), adding one segment from the last node to the closing node, and no node is added. This closes a new path as a created path object, and a continued path as the same object P (`close_path`, criterion 19).
15. Given the closing node and the join type, then:
    - **Default (no Shift) is "as drawn":** if the closing node is a Symmetric or Asymmetric node the join is Smooth; if it is a Corner node the join is Sharp. A path drawn with clicks therefore closes as it does today.
    - **Shift flips it:** Smooth becomes Sharp and Sharp becomes Smooth. Shift is read live; pressing or releasing it with the pointer at rest updates the chip and the preview at once (`0014`, live modifier rule).
    - **Applying Sharp:** a Symmetric or Asymmetric closing node becomes a Corner node whose handle **on the closing segment's side is retracted to zero** (the node's incoming handle when the closing segment runs from the last node to the closing node, which is the case for a new path, for the Close path command, and for a continuation from P's last node; its outgoing handle when a continuation from P's first node closes onto P's last node, because the stored order is then reversed). The other handle stays exactly where it is. The result is a cusp with no handle on the closing side: the closing segment arrives at the node without a tangent, so Sharp and Smooth never draw the same curve. (Leaving both handles in place would draw the same curve as Smooth, because two collinear handles on a Corner node look like a Symmetric node; so the preview and the result of Shift would be identical for a path whose first node was drawn with a drag.) A Corner node is unchanged.
    - **Applying Smooth:** a Corner closing node becomes Asymmetric by `0006` criterion 2: two handles collinear through the node along the path's local tangent, computed as for the closed path (the tangent is the direction from the previous node of the closing segment to the node after the closing node), each handle keeping its length if it already had one and taking the default handle length otherwise. A Symmetric or Asymmetric node is unchanged. No other node changes.
    Test: nodes A (0, 0), B (20, 0), C (20, 20), D (0, 20), all Corner and without handles, drawn by clicks; closing onto A with no Shift gives a closed square with four Corner nodes; closing with Shift makes A Asymmetric with collinear handles along the direction from D to B and the segments D to A and A to B become curves; closing a path whose first node was drawn with a drag (Symmetric) without Shift leaves it Symmetric; with Shift it becomes a Corner whose closing-side (incoming) handle has length zero and whose other (outgoing) handle is as drawn, and the segment from the last node to the first arrives at the first node as a curve with no handle on that end. The two results (with and without Shift) draw different closing segments; the test compares them.
16. Given the pointer is over the closing target, then the two-line hint chip appears at once and reads, by criterion 15's default, "Close path with a sharp corner" / "Shift: smooth curve" or "Close path with a smooth curve" / "Shift: sharp corner"; while Shift is down it reads the swapped pair with the second line "Release Shift: smooth curve" or "Release Shift: sharp corner" (the join that releasing Shift gives back). The chip is text only, takes no focus, is `aria-hidden`, and is placed once, 12 px up and right of the pointer at the moment it appears (flipping at the viewport edges, as the transform handle hint chip does); it does not follow the pointer while the pointer stays on the same target. It disappears on leaving the target, on the press, and on any key other than Shift.
17. Given the pointer is over the closing target, then the rubber band is replaced by a preview of the closing segment exactly as it will be committed with the resolved join (a dashed `--accent` curve or line), the handles of the closing node that the join gives are drawn, and the closing node is drawn as the glyph of its **resolved kind** (hollow `--accent` outline: square for Corner, diamond for Symmetric, triangle for Asymmetric), so Sharp reads as a square with no handle on the closing side and Smooth as a diamond or triangle with two handles in line. It updates in the frame Shift changes. It is not stored. The same pure function resolves the join for the preview and for the commit, so they cannot disagree (`0002` adrs, "commands carry resolved geometry").

### Part D: close an existing path

18. Given the Node tool, then at the end of the Node bar, after a divider, there is a group "Close path" (`role="group"`): the label "Close path" and two text buttons, **Sharp** and **Smooth** (accessible names "Close path, sharp corner" and "Close path, smooth curve"; two buttons, one gesture each; commands, not toggles: no pressed state, no mode, nothing remembered). The group exists in the Node tool only; the Select bar has none (a maker in the Select tool presses `N` or double-clicks the path). The buttons are always present and are dimmed (`aria-disabled`, still focusable, inert, tooltip states the reason) when the editing set holds no open ordinary path with 3 or more nodes; which nodes are selected does not matter. Pressing a dimmed button does nothing. Keyboard: Tab reaches both in the bar's order, Space and Enter press, no letter shortcut.
19. Given the command, then every open path of the editing set with 3 or more nodes gets one segment from its last node to its first node, and its first node is the closing node with the chosen join type applied as in criterion 15, by the same function as the Pen (Sharp or Smooth, no "as drawn" and no Shift). Paths with fewer than 3 nodes are skipped. It is one commit, stored as `close_path`, for all of them, atomic. A path whose first and last node are within 0.001 mm of each other has them merged into one node first (`0006` criterion 10), and the join applies to the merged node.
20. Given the command ran, then the closed paths keep their object id, style, place and selection; nodes that were selected stay selected if they still exist. A notice of one line says what happened: "Closed 3 paths. No undo yet." or, when something was skipped, "Closed 2 paths. 1 path has fewer than 3 nodes and was not closed. No undo yet." It is anchored 4 px below the pressed button, right-aligned to it and clamped to the viewport, takes no focus, is a `role="status"` live region (as `0016` criterion 29), and disappears after 3 seconds, or after 5 seconds when the sentence is longer than 70 characters (the skipped variant has 78).
21. Given a compound path, a primitive or an already closed path in the editing set, then it is not closed and not counted as skipped.

### Part E: cursors, tolerances, Escape, limits

22. Given the Pen tool, then it has four cursors: the Pen nib (new path, as today), the close variant (as today), a continue variant and a join variant; the last two are new (24 px, white halo under black, hotspot at the nib tip, `crosshair` where custom images are ignored; continue: the nib with a short stub ending in a solid dot at the lower right; join: the nib with two dots joined by a bar; the four are told apart at 1x on a glyph sheet before the PR is accepted). The cursor, the node and the chip always agree with what the press would do. With Shift held over a continue or join target the cursor is the plain nib; over the close target the close cursor stays. The Select and Node tools' cursors do not change.
23. Given the hit tolerances, then every target in this spec (the hover cue, the press, the close target, the connect target) uses the node hit radius of 16 px, not the 8 px object tolerance of the Select tool and not the 4 px segment tolerance of the Node tool.
24. Given the Pen is idle, then the hover test looks at the end nodes of open paths only. With 5000 open paths in the document, a pointer move costs under 2 ms in a release build on the reference desktop, with the end-node cache warm (an index of the end nodes of open ordinary paths, rebuilt when `Document::version()` changes). The first pointer move after a document change pays one full read of the document's paths to rebuild the index; that cost is not under the 2 ms budget (tens of ms at 5000 paths, the same read every frame already pays). The implementer measures both numbers with an `#[ignore]` release benchmark and reports them in the PR.
25. Given Continuing, when Escape is pressed, then the new nodes are discarded, P is exactly as it was (unchanged object, no commit), and the Pen is idle; the next Escape switches to the Select tool (`0010` criterion 43). Given a click-drag of a new node in progress, Escape discards the whole addition in the same press, as for a new path. In the same way, Escape on a new path that was about to join Q discards the new path and leaves Q alone.
26. Given the maker switches tool, presses a rail button or opens the Properties panel's controls while Continuing, then the continuation is finished or discarded the way an unfinished new path is today (`0015` criterion 14a as-built note: it ends as drawn when the Pen is left). It is never left half-written.
27. Given this slice, then no undo exists (`0020-undo-redo`). Each gesture is one commit so undo can take it later; the discard-by-Escape in criterion 25 is the only way back before finishing.

## Changes to accepted behaviour

- `0002` Out of scope, "Extending an already-finished open path by clicking back onto one of its endpoints with the pen tool ... the pen tool always starts a new path object": **superseded.** A press on an end node now continues the path, unless Shift is held.
- `0002` criterion 5 (closing) and its UX notes: the close cue gains the join text and the preview; the geometry of a close with no Shift is unchanged (criterion 15, "as drawn").
- `0010` criterion 39 (Shift and Ctrl have no new meaning in the Pen): superseded for Shift at the three targets above.
- `0015` criterion 14a: "unfinished path" includes Continuing.

## Out of scope

- **Dragging on the closing or the end node to shape its handle** (Illustrator and Inkscape do this); the join type is chosen by the key and the command. Question 6.
- **Continuing or connecting from an interior node** (branching), Inkscape's append-to-selected-path with Shift.
- **Snapping a finished path's end to a nearby end** without a click on it, and closing by distance when the path is finished with Enter.
- **Reversing a path's direction** as a command, and choosing the direction on connect.
- **Compound paths, primitives and groups** as targets (a path inside a group becomes a target only if `0023-groups` says so).
- **Alt and Ctrl at the targets.** Ctrl is kept for angle constraint in the Pen, which the product does not have yet.
- **A keyboard way to close** (a "C" key), a shortcut for Close path.
- **Close path in the Select bar or the left rail.** The command lives in the Node bar only (UX decision, 2026-10-10).
- **Merging Q's style into P**, choosing the surviving style in a prompt.
- **Undo and redo** (`0020`).

## Open questions

Each has a default; nothing blocks.

1. **Default join (criterion 15).** *A (default):* "as drawn": Corner closes sharp, Symmetric or Asymmetric closes smooth, Shift flips. No behaviour changes for anyone who does not press Shift. *B:* always Sharp by default. *C:* always Smooth by default. Recommendation: A.
2. **The key (criterion 15).** *A (default):* Shift. It is free in the Pen (`0010` criterion 39), it is "the other option" in Inkscape's and Illustrator's Pen, and one rule serves the three targets. *B:* Alt: avoided because many Linux window managers take Alt+click and Alt+drag for moving windows, and Alt is the marquee and cycling key of `0014`. *C:* Ctrl: kept for angle constraint later. Recommendation: A.
3. **Continue any path or only the selected one (criterion 6).** *A (default):* any open path (Illustrator), with Shift to start a new path instead. *B:* only the selected path (Inkscape): safer against an accidental continuation, but the maker must select first and the ring does not show on other paths. Recommendation: A, because the cue, Shift and Escape make an accident cheap.
4. **Connect: a segment or a merge (criterion 9).** *A (default):* a segment between the two ends, except when they coincide. *B:* always merge the ends into one node as Join does (`0006`), moving a node. Recommendation: A, because the maker clicked each end where they want it.
5. **Command names (criterion 18).** Default: "Close path" with the buttons "Sharp" and "Smooth". Option: "Close with corner" and "Close smooth".
6. **Drag at the close point.** Default: not built. Say if the Illustrator gesture (drag out of the closing node to set its handle) should join the key and the command.
7. **Where the command lives.** Resolved by the ux-engineer: the Node bar only (criterion 18); the Select bar group and the rail were rejected (see the UX notes).

Decided by the product owner (change if you disagree): the surviving style is the continued path's (the first path); P's id and place survive a join; a new path ending on Q becomes Q; the closing node is the node clicked; Smooth means Asymmetric for a Corner node (keeps handle lengths, changes least); the commit labels `extend_path`, `connect_paths`, `close_path`; the 16 px radius; the chip wording; four Pen cursors; no selection needed to continue.

## UX notes

By `ux-engineer`, 2026-10-10. Rows are in `docs/design-system.md`: "Pen target cue", "Pen hint chip", "Pen cursors" (Interaction conventions), "Close path group" (Node bar) and the amended "Action notice". Reference tools: Illustrator signals continue and join with a cursor change only; Inkscape signals nothing before the click. Here the cursor, the node and a short chip agree, the chip says what Shift will do, and the closing segment is drawn before the click. The screen stays calm: nothing new is drawn unless the pointer is within 16 px of a node the press would act on.

### What shows over a target

| State of the Pen | Node | Cursor | Chip (no Shift) | Chip (Shift held) | Line on canvas |
|---|---|---|---|---|---|
| Idle, over an end node of an open path | node glyph and 18 px ring | continue | "Continue path" / "Shift: start a new path" | "Start a new path" / "Release Shift: continue path" | none |
| Drawing, over an end node of another open path | node glyph and ring | join | "Join with path" / "Shift: place a node" (+ style line, below) | "Place a node" / "Release Shift: join with path" | rubber band ends on the node's centre |
| Drawing, over the close target (first node; for a continuation the other end F) | the existing ring on the drawn node | close | "Close path with a sharp corner" / "Shift: smooth curve", or with the two swapped when the default is smooth | the swapped pair, second line "Release Shift: ..." | the closing segment, dashed, replaces the rubber band |
| Anywhere else | none | nib | none | none | rubber band to the pointer |

With Shift held over a continue or join target the **ring is gone and the cursor is the plain nib**: the cue shows exactly what the press will do (a new path, a plain node). Over the close target Shift changes the text and the drawn segment, not the cursor: it is still a close.

- **Node glyph with the ring.** The hover ring alone is `--accent` at 20% and measures about 1.2:1 on the canvas, and the Pen draws no nodes of committed paths, so a ring around nothing would be nearly invisible. A target end node is therefore drawn with its idle node glyph (14 px, white fill, `--node-stroke` outline, the shape of its kind) inside the 18 px `--accent-hover` ring. The glyph carries the contrast; the ring keeps the family look. The permanent ring of a continuation's start node E is unchanged (`0002`).
- **Cursors** (four, 24 px, white halo under black like the rotate and lasso cursors, hotspot at the nib tip, `crosshair` where custom images are ignored): the Pen nib (new path, as built); **close**, the nib with a small hollow circle at the lower right (as built); **continue**, the nib with a short stub ending in a solid 4 px dot at the lower right (a line that goes on); **join**, the nib with two solid 3 px dots 4 px apart joined by a 2 px bar (a link). No feature under 2 px; the four must be told apart at 1x on a glyph sheet before the PR is accepted. The cursor and the node change in the same frame.
- **Cursors during a press:** unchanged from the cue until release; a drag from a target is ignored (criteria 2, 8, 14) and the cursor stays what it was.

### The hint chip

The surface of the "Transform handle hint chip" (`--toolbar-bg` / `--toolbar-icon`, 12 px, 8 px padding and radius, `pointer-events: none`), two lines: the action in semibold, then the Shift line. It is **placed once**, 12 px up and right of the pointer at the moment it appears (readout placement, flips at the edges), and stays there while the pointer remains on the same target; it does not follow the pointer inside the 16 px radius, because text that drifts under the cursor is hard to read and the target is small.

- **Delay.** Continue and Join: **600 ms of rest on the target**, or at once when Shift is pressed or released over the target (the maker is asking). The ring and the cursor are immediate. Reason: a Pen moving through a drawing with many open ends would flash a chip at every end it passes. Close: **at once**, because it carries the choice and the one-way commit, and its preview is drawn at once too.
- **Gone** on leaving the target, on the press, and on any key except Shift. Escape removes it with the rest of the state.
- **Style line.** Only when joining and the two paths differ in style, and only when the maker is continuing a path P (a new path that ends on Q becomes Q, so nothing is lost): a third muted line, "The result keeps the style of the path you continue." The press is the commit for a join, so this is the maker's only warning that Q will change its colour or width.
- **WebKitGTK:** the Shift key-up is not delivered (`0014` plan.md), so after Shift is released the chip, the ring and the preview keep the Shift state until the next pointer event. The press reads the modifier from its own event and the cue is redrawn in the press frame, so the rare mismatch (release Shift, click without moving) shows the right text for the length of the click. Known, not a blocker.

### Closing: Sharp and Smooth, shown before the click

- **Default and flip:** criterion 15 as written, with the change below. Shift is read live; the chip, the drawn segment and the node glyph update in the frame Shift changes.
- **Preview** (criterion 17): over the close target the rubber band becomes the closing segment as it will be committed (1 px dashed `--accent`), with the resolved closing node's handles in their idle look (1 px `--accent` line, 12 px white circle with `--accent` ring) and the node drawn as the **glyph of the resolved kind** (hollow `--accent` outline as every in-progress node: square for Corner, diamond for Symmetric, triangle for Asymmetric). So Sharp reads as a corner with no handle on the closing side and a square, Smooth as a curve with two handles lined up and a triangle or diamond. A maker who never reads the chip still sees which one they get.
- **Finding, requested change to criterion 15.** As written, Sharp on a Symmetric closing node "becomes a Corner with both handles left exactly where they are". Two collinear handles on a Corner node draw the same curve as on a Symmetric node, so for a path whose first node was drawn with a drag the preview and the result would be **identical** whether Shift is held or not, and the chip would promise a sharp corner that is not there. Sharp must retract the handle on the closing segment's side (set it to zero, so the segment arrives without a tangent) and keep the other handle. A Corner node is unchanged; Smooth is unchanged. The Close path command (criterion 19) gets the same rule through the same function.

### Joining paths of different styles

The surviving style rule is criterion 10. The maker sees it in three places: the chip line above before the press, the result at once after it (Q takes the style of P), and nothing else: no notice, because the Pen shows none for any gesture and the visible change is the feedback. Which node ends up where is not explained on screen; the rule is in the spec and in the user documentation, not in the UI.

### Escape

The chip, the ring and the dashed closing segment are part of the Pen's state and go with it. First Escape while Continuing, or while a path that was about to join is in progress: the new nodes disappear, P and Q are drawn as they were, the Pen is idle with the plain nib, no notice (nothing was written). Second Escape: the Select tool (`0010`). During a click-drag of a node: the whole addition is discarded in the same press (criterion 25). Escape never closes, joins or finishes anything.

### Close path (Part D): where it lives

**Decision: the Node bar, as a group at its end; not the rail, not the Select bar, not the panel.**

- *Not the Path card of the rail.* The rail is full at 22 buttons and 20 are planned (`Tool rail architecture`). Two more would use the whole spare capacity (20 planned, 22 the cap) for a command that applies to open paths only, and the words Sharp and Smooth do not fit an icon-only card. The Path card is for commands that change objects as wholes.
- *Not the Select bar.* It already wraps to three rows at 800 x 600 and is crowded by kind groups. The command is about nodes (it sets the kind of the closing node), and the Node tool is where node kinds live (the 3-way kind control, Join, Split). A maker in the Select tool presses `N` or double-clicks the path; the Select bar group of the proposal is dropped (criterion 18 change).
- *Not the panel.* A section that comes and goes at the top of the Style area shifts every row below it.

**The group.** After a divider at the end of the Node bar: the label "Close path" (14 px `--toolbar-icon`, not a control) and two text buttons, **Sharp** and **Smooth** (28 px high, 1px `--toolbar-icon` at 60% outline, `rounded-[5px]`, 14 px label, 12 px side padding, 4 px apart; the "Fit to content" button look; hover `--editor-accent-hover`, focus ring as the bar's). The group is `role="group"`, name "Close path"; the buttons' names are "Close path, sharp corner" and "Close path, smooth curve" (they contain the visible word). About 208 px, 233 px with its divider. They are commands, not toggles: no pressed state, nothing remembered.

- **Enabled / dimmed.** The Node bar's own rule (`0002`, `0006`): always present, greyed and inert when they do not apply, implemented as Join and Split are, so the bar has one behaviour. Applies when the editing set holds at least one open ordinary path with 3 or more nodes; the nodes selected do not matter. A dimmed button still shows its tooltip on hover and focus (the reason is the content).
- **Tooltips** (`side="bottom"`, 400 ms, 260 px, three lines): "Close path with a sharp corner" / "Joins the last node to the first. The first node becomes a corner." / note; and "Close path with a smooth curve" / "Joins the last node to the first. The first node becomes smooth." / note. Note by state: nothing applicable "Select an open path with 3 or more nodes."; otherwise "Closes 2 open paths. No undo yet."; with a skip "Closes 2 of 3 open paths. 1 has fewer than 3 nodes. No undo yet.".
- **After the press:** the closing segment appears, the first node shows its new kind glyph, selections are kept, a filled path fills. The one-line notice of criterion 20 appears 4 px below the pressed button, right-aligned to it, clamped to the viewport (the panel-button rule of the Action notice): "Closed 3 paths. No undo yet." or "Closed 2 paths. 1 path has fewer than 3 nodes and was not closed. No undo yet.", `role="status"`, no focus, no other control touched. A mouse press returns focus to the canvas as for the other Node bar buttons; a key press leaves it on the button.
- **Width.** The Node bar is about 300 px today; with the group about 530 px. The overlay row at 800 x 600 with the panel open and two rail columns is **356 px** (`0035` UX notes), so the group wraps to a second row of its own at that size and the bar is two rows (72 px) whenever the Node tool is active; from about 1000 px window width with the panel open (row 556 px), and with the panel collapsed, it is one row. This is accepted: the alternative is hiding the group, which would move the centred bar's buttons. The implementer measures both bars at build and reports the numbers; if the Node bar's other groups are wider than assumed here, the same rule (wrap by whole groups) applies.
- **Keyboard:** Tab reaches both buttons in the bar's order (after Make line / Make curve); Space and Enter press; no letter shortcut (no undo yet).

### Accessibility summary

The Pen's gestures are pointer-only as the Pen is; the keyboard routes are the Close path buttons and, for connecting two paths, Join in the Node tool (`0006`). The hint chips are `aria-hidden` (a screen reader cannot use the canvas they describe, and a live region on hover would chatter); the Close path notice is a `role="status"` region. State is never colour alone: cursor shape, node glyph, ring and chip text agree. Chip contrast is the chip surface's 8.3:1; the dimmed bar buttons are exempt. Hit radius 16 px for every target (criterion 23). No motion.

### Open design questions (defaults taken)

1. **Close path only in the Node bar.** *Default:* yes. *Alternative:* also a group in the Select bar when the selection holds an open path (the proposal of criterion 18); costs width in the most crowded bar.
2. **Chip delay of 600 ms for Continue and Join.** *Default:* yes. *Alternative:* immediate, which flashes at every end the Pen passes.
3. **Sharp retracts the closing-side handle** (finding above). *Default:* yes; without it Sharp is invisible for drawn-with-a-drag nodes.
4. **The Node bar is two rows at 800 x 600 with the panel open.** *Default:* accept. *Alternative:* a single icon pair (about 70 px) with the words in the tooltip; cheaper in width, but the criterion asks for the words on the buttons.
5. **The style line on Join.** *Default:* yes (no undo, and Q changes its look).

### Criteria changes requested (by number)

Applied to the criteria above on 2026-10-10: 1 and 7 (two-line chips, 600 ms rest, glyph in the ring, no ring with Shift, the third style line), 15 (Sharp retracts the closing-side handle; this was a defect: with both handles kept, Sharp looked identical to Smooth), 16 (texts, placed once), 17 (glyph of the resolved kind), 18 (Node bar only, dimmed not removed), 19 (Sharp follows 15), 20 (notice anchor and duration), 22 (Shift cursor), 24 (cache and `Document::version()`, architect).

## Delivery

Milestone M3 of the path-tools slice (`story/pen-path-extension` milestones in `adrs.md`), delivered in one PR together with the toolbox fix of `0016-boolean-operations`, `0031-segment-drag-bending` and `0035-combine-and-break-apart` (customer rule: one PR per slice). It starts after 0031 is built on the branch (shared files in `adrs.md`) and before 0035.

## Links

Requirements: R-EDIT-022 (`docs/requirements.md`); related R-EDIT-001, R-EDIT-014
Builds on: `specs/0002-path-node-editing/` (Pen, close, hover cues), `specs/0006-path-merge-split-and-node-types/` (Join, node kinds), `specs/0010-edit-interaction-polish/` (Escape cascade), `specs/0014-advanced-selection/` (live modifier keys)
Related: `specs/0016-boolean-operations/` (compound paths excluded), `specs/0023-groups/` (targets inside groups), `specs/0015-document-size-and-rulers/` (criterion 14a)
ADRs: `adrs.md` (architect, 2026-10-10)
PR: TBD (shared path-tools PR)
