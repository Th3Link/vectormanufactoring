/**
 * The words of the stretch that turns shapes into paths
 * (`specs/0019-multi-object-transform/` criteria 23, 34, 54, 55). Rust sends the
 * per-kind counts, `[polygons, stars, rotated rectangles, rotated ellipses]`; the
 * sentences are written here, once. Only the total is named: the kinds do not
 * change the wording ("shapes", the word the Select bar uses).
 */

/** The counts as `WasmSession.hover_conversion_counts` and its siblings send them:
 * empty when nothing converts. */
export type ConversionCounts = ArrayLike<number>;

/** How long the notice of a commit stays (criterion 54: more than the 2 s of a key
 * hint, since the change cannot be undone). */
export const CONVERSION_NOTICE_MS = 6000;

/** All shapes counted. */
export function totalConverted(counts: ConversionCounts): number {
  let total = 0;
  for (let index = 0; index < counts.length; index += 1) {
    total += counts[index] ?? 0;
  }
  return total;
}

/** "2 shapes into paths" / "1 shape into a path". */
function intoPaths(total: number): string {
  return total === 1 ? "1 shape into a path" : `${total} shapes into paths`;
}

/** The line of an edge handle's hint chip, after its title (criterion 55);
 * `null` when nothing converts. */
export function stretchHoverLine(counts: ConversionCounts): string | null {
  const total = totalConverted(counts);
  return total === 0 ? null : `Stretching turns ${intoPaths(total)}`;
}

/** The second line of the scale readout while the live result converts
 * (criterion 23); `null` when it converts nothing. */
export function stretchReadoutLine(counts: ConversionCounts): string | null {
  const total = totalConverted(counts);
  if (total === 0) {
    return null;
  }
  return total === 1 ? "1 shape becomes a path" : `${total} shapes become paths`;
}

/** The notice after a commit that converted shapes (criterion 54), with the count
 * as plain digits; `null` when it converted nothing. */
export function stretchNotice(counts: ConversionCounts): string | null {
  const total = totalConverted(counts);
  return total === 0
    ? null
    : `Stretching turned ${intoPaths(total)}. No undo yet.`;
}

/** The note under the fields of a size entry while the typed size is a stretch
 * (criterion 34); `null` when it is not. */
export function stretchEntryNote(counts: ConversionCounts): string | null {
  const total = totalConverted(counts);
  return total === 0 ? null : `Turns ${intoPaths(total)}.`;
}
