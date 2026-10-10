/**
 * The words of the stretch that turns shapes into paths
 * (`specs/0019-multi-object-transform/` criteria 23, 34, 54, 55). Rust sends one
 * count of shapes (0 when nothing converts); the sentences are written here, once.
 * The kinds are not named: "shapes" is the word the Select bar uses.
 */

/** How long the notice of a commit stays (criterion 54: more than the 2 s of a key
 * hint, since the change cannot be undone). */
export const CONVERSION_NOTICE_MS = 6000;

/** "2 shapes into paths" / "1 shape into a path". */
function intoPaths(total: number): string {
  return total === 1 ? "1 shape into a path" : `${total} shapes into paths`;
}

/** The line of an edge handle's hint chip, after its title (criterion 55);
 * `null` when nothing converts. */
export function stretchHoverLine(total: number): string | null {
  return total === 0 ? null : `Stretching turns ${intoPaths(total)}`;
}

/** The second line of the scale readout while the live result converts
 * (criterion 23); `null` when it converts nothing. */
export function stretchReadoutLine(total: number): string | null {
  if (total === 0) {
    return null;
  }
  return total === 1 ? "1 shape becomes a path" : `${total} shapes become paths`;
}

/** The notice after a commit that converted shapes (criterion 54), with the count
 * as plain digits; `null` when it converted nothing. */
export function stretchNotice(total: number): string | null {
  return total === 0
    ? null
    : `Stretching turned ${intoPaths(total)}. No undo yet.`;
}

/** The note under the fields of a size entry while the typed size is a stretch
 * (criterion 34); `null` when it is not. */
export function stretchEntryNote(total: number): string | null {
  return total === 0 ? null : `Turns ${intoPaths(total)}.`;
}
