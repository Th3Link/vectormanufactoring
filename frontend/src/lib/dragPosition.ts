/** Where a value-field drag is on the scale (`specs/0017-style-panel-rework`
 * criteria 38 and 44): no React, so a test can drive it. */

/** What a drag knows about itself. */
export interface DragFrame {
  /** The edited objects differed when the press happened. Latched there: the
   * preview makes them equal after the first tick, but the drag stays absolute. */
  mixed: boolean;
  /** The position and pointer x the factor is applied from (relative drags). */
  baseP: number;
  baseX: number;
  factor: number;
  /** The field's left edge and width, px. */
  left: number;
  width: number;
}

/** The position `p`, 0 to 1, for the pointer at `clientX`. A mixed drag takes
 * the pointer's own position in the field; any other moves from the value at
 * the press, scaled by the modifier, clamped without re-basing at the ends. */
export function dragPosition(frame: DragFrame, clientX: number): number {
  const raw = frame.mixed
    ? (clientX - frame.left) / frame.width
    : frame.baseP + (frame.factor * (clientX - frame.baseX)) / frame.width;
  return Math.min(1, Math.max(0, raw));
}
