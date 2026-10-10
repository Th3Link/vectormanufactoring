import { useId } from "react";

import { cssColor } from "@/lib/styleColor";

interface SwatchProps {
  /** Packed `0xRRGGBB`. */
  rgb: number;
  /** Opacity percent. */
  opacity: number;
  /** The edited objects differ: a 45 degree hatch, never a checkerboard. */
  mixed: boolean;
  /** Edge length, px (28 in a row, 16 in the eyedropper chip). */
  size?: number;
}

/**
 * A colour swatch (`docs/design-system.md`, "Swatch"): the colour at its
 * opacity over a checkerboard, a 1 px `--swatch-border` and a 1 px white inner
 * line so any colour is bounded against the panel. Mixed shows a hatch. Display
 * only (`specs/0017-style-panel-rework` criterion 16): hidden from the
 * accessibility tree, no hover state, not a tab stop.
 */
export function Swatch({ rgb, opacity, mixed, size = 28 }: SwatchProps) {
  const id = useId();
  const cell = 7;
  return (
    <span
      aria-hidden
      className="relative block shrink-0 overflow-hidden rounded-[5px] border border-[var(--swatch-border)]"
      style={{ width: size, height: size }}
    >
      <svg aria-hidden className="absolute inset-0 size-full">
        <defs>
          <pattern id={`${id}-checker`} width={cell * 2} height={cell * 2} patternUnits="userSpaceOnUse">
            <rect width={cell * 2} height={cell * 2} fill="var(--checker-a)" />
            <rect width={cell} height={cell} fill="var(--checker-b)" />
            <rect x={cell} y={cell} width={cell} height={cell} fill="var(--checker-b)" />
          </pattern>
          <pattern
            id={`${id}-hatch`}
            width="8"
            height="8"
            patternUnits="userSpaceOnUse"
            patternTransform="rotate(45)"
          >
            <rect width="8" height="8" fill="var(--checker-a)" />
            <rect width="4" height="8" fill="var(--mixed-hatch)" />
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill={`url(#${id}-${mixed ? "hatch" : "checker"})`} />
      </svg>
      {!mixed && (
        <span className="absolute inset-0" style={{ background: cssColor(rgb, opacity) }} />
      )}
      <span aria-hidden className="absolute inset-0" style={{ boxShadow: "inset 0 0 0 1px #fff" }} />
    </span>
  );
}
