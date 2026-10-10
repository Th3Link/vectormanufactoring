/** The texts and times of the editing quick wins (`specs/0044-editing-quick-wins/`). */

/** The move readout of a nudge goes this long after the last move (criterion 9). */
export const NUDGE_READOUT_MS = 800;

/** A held key whose repeat events stop for this long has ended its step (criterion 9); the same
 * window as `CONTINUATION_WINDOW_MS` in `curvyo-ui-core`. */
export const NUDGE_RUN_END_MS = 600;

/** The muted line under the Select tool's tooltip (criterion 14). */
export const SELECT_KEYS_TOOLTIP = "Arrows nudge 1 mm, Shift 10 mm. Ctrl+A selects all.";

/** The key hint of a nudge past the coordinate limit (criterion 10). */
export const TOO_FAR_HINT = "Too far from the document. Nothing was changed.";

/** What a screen reader hears after Ctrl+A (criterion 4); nothing for an empty selection. */
export function selectedAnnouncement(count: number): string {
  if (count <= 0) {
    return "";
  }
  return count === 1 ? "Selected 1 object." : `Selected ${count} objects.`;
}
