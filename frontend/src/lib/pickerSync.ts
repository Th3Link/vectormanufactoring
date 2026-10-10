/** What the colour picker remembers of the colour it last read from outside. */
export interface PickerSeen {
  /** The packed `0xRRGGBB` it last read, or `null` before the first read. */
  rgb: number | null;
  /** The function that converted it (`hsvOf`). A different one means the session behind it
   * changed, for example from "not there yet" to the real one. */
  source: unknown;
}

/** What the picker does with the colour it is given. */
export type PickerStep =
  /** Derive hue, saturation and value from the colour again, and remember it. */
  | "derive"
  /** Remember the colour; the picker's own state already shows it. */
  | "note"
  /** Nothing changed. */
  | "keep";

/**
 * The rule of the inline picker (`specs/0017-style-panel-rework` criterion 20): it keeps its
 * own hue, saturation and value and re-derives them only when the colour changed from outside
 * it, so the hue does not jump through a grey. It also re-derives when the converter changed:
 * the Background block mounts before the wasm session exists, with a placeholder converter and
 * the same default colour the real one reports, so the colour alone never tells it to start
 * over (`specs/0040-document-background`, UX review B1).
 *
 * @param seen what it last read
 * @param rgb the colour now given
 * @param own the colour the picker itself last sent, or `null`
 * @param source the converter now in use
 * @param mixed the edited objects differ: the picker shows no thumb and reads nothing
 */
export function pickerStep(
  seen: PickerSeen,
  rgb: number,
  own: number | null,
  source: unknown,
  mixed: boolean,
): PickerStep {
  if (mixed) {
    return "keep";
  }
  if (seen.source !== source) {
    return "derive";
  }
  if (seen.rgb === rgb) {
    return "keep";
  }
  return own === rgb ? "note" : "derive";
}
