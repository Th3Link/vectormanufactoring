// Colour and number text helpers of the Style panel. The rules (what is a
// valid hex, how an opacity rounds) are Rust's (`curvyo-ui-core`'s
// `style_entry`); this file only formats values for display and converts
// between the packed 0xRRGGBB the session reports and the HSV the picker
// works in.

/** The hue/saturation/value/alpha of the picker; hue 0 to 360, the rest 0 to 100. */
export interface Hsva {
  h: number;
  s: number;
  v: number;
  a: number;
}

/** `#RRGGBB`, upper case. */
export function toHex(rgb: number): string {
  return `#${(rgb & 0xff_ff_ff).toString(16).toUpperCase().padStart(6, "0")}`;
}

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

export function rgbToHsv(rgb: number): { h: number; s: number; v: number } {
  const r = ((rgb >> 16) & 0xff) / 255;
  const g = ((rgb >> 8) & 0xff) / 255;
  const b = (rgb & 0xff) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;
  let h = 0;
  if (delta > 0) {
    if (max === r) {
      h = ((g - b) / delta) % 6;
    } else if (max === g) {
      h = (b - r) / delta + 2;
    } else {
      h = (r - g) / delta + 4;
    }
    h *= 60;
    if (h < 0) {
      h += 360;
    }
  }
  return { h, s: max === 0 ? 0 : (delta / max) * 100, v: max * 100 };
}

export function hsvToRgb(h: number, s: number, v: number): number {
  const sat = s / 100;
  const val = v / 100;
  const chroma = val * sat;
  const sector = (((h % 360) + 360) % 360) / 60;
  const x = chroma * (1 - Math.abs((sector % 2) - 1));
  const [r1, g1, b1] =
    sector < 1
      ? [chroma, x, 0]
      : sector < 2
        ? [x, chroma, 0]
        : sector < 3
          ? [0, chroma, x]
          : sector < 4
            ? [0, x, chroma]
            : sector < 5
              ? [x, 0, chroma]
              : [chroma, 0, x];
  const m = val - chroma;
  const channel = (c: number) => Math.round((c + m) * 255);
  return (channel(r1) << 16) | (channel(g1) << 8) | channel(b1);
}
