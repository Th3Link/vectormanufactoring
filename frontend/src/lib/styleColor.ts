// Colour and number text helpers of the Style panel. The rules (what is a
// valid hex, how an opacity rounds, hue and saturation) are Rust's
// (`curvyo-ui-core`'s `style_entry` and `colour_hsv`); this file only formats
// values for display.

/** The CSS colour of a packed colour at an opacity percent. */
export function cssColor(rgb: number, percent: number): string {
  const r = (rgb >> 16) & 0xff;
  const g = (rgb >> 8) & 0xff;
  const b = rgb & 0xff;
  return `rgba(${r}, ${g}, ${b}, ${Math.min(1, Math.max(0, percent / 100))})`;
}
