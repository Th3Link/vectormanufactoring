// Colour and number text helpers of the Style panel. The rules (what is a
// valid hex, how an opacity rounds, hue and saturation) are Rust's
// (`curvyo-ui-core`'s `style_entry` and `colour_hsv`); this file only formats
// values for display.

/** An opacity percent as the integer the field shows. */
export function percentText(percent: number): string {
  return String(Math.round(percent));
}

/** A width in millimetres: up to three decimals, no trailing zeros ("0.125").
 * A positive width that three decimals would show as "0" keeps enough digits to
 * show it ("0.0004"), so a stroke that is on never reads as no stroke. */
export function widthText(mm: number): string {
  const rounded = Math.round(mm * 1000) / 1000;
  if (rounded !== 0 || !(mm > 0)) {
    return String(rounded);
  }
  const digits = Math.min(20, 2 - Math.floor(Math.log10(mm)));
  return mm.toFixed(digits).replace(/0+$/, "");
}

/** The CSS colour of a packed colour at an opacity percent. */
export function cssColor(rgb: number, percent: number): string {
  const r = (rgb >> 16) & 0xff;
  const g = (rgb >> 8) & 0xff;
  const b = rgb & 0xff;
  return `rgba(${r}, ${g}, ${b}, ${Math.min(1, Math.max(0, percent / 100))})`;
}
