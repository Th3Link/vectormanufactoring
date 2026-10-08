import { forwardRef } from "react";
import type { ButtonHTMLAttributes } from "react";

import { cssColor } from "@/lib/styleColor";

const CHECKER =
  "linear-gradient(45deg, var(--checker-b) 25%, transparent 25%, transparent 75%, var(--checker-b) 75%), linear-gradient(45deg, var(--checker-b) 25%, transparent 25%, transparent 75%, var(--checker-b) 75%)";

interface SwatchProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Packed `0xRRGGBB`. */
  rgb: number;
  /** Opacity percent. */
  opacity: number;
  /** The edited objects differ: a 45 degree hatch, never a checkerboard. */
  mixed: boolean;
  /** The paint is off: the stored colour with a slash across it. */
  off?: boolean;
  /** 28 px in a row, 24 px in a stop row. */
  size?: 28 | 24;
}

/**
 * A colour swatch (`docs/design-system.md`, "Swatch"): the colour at its
 * opacity over a checkerboard, a 1 px `--swatch-border` and a 1 px white inner
 * line so any colour is bounded against the panel. Mixed shows a hatch; an off
 * paint shows the slash.
 */
export const Swatch = forwardRef<HTMLButtonElement, SwatchProps>(function Swatch(
  { rgb, opacity, mixed, off = false, size = 28, className = "", ...props },
  ref,
) {
  const cell = size === 28 ? 7 : 6;
  return (
    <button
      ref={ref}
      type="button"
      className={`relative shrink-0 overflow-hidden rounded-[5px] border border-[var(--swatch-border)] bg-[var(--checker-a)] outline-none focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--toolbar-bg)] ${className}`}
      style={{
        width: size,
        height: size,
        backgroundImage: mixed ? undefined : CHECKER,
        backgroundSize: `${cell * 2}px ${cell * 2}px`,
        backgroundPosition: `0 0, ${cell}px ${cell}px`,
      }}
      {...props}
    >
      <span
        aria-hidden
        className="absolute inset-0"
        style={{
          background: mixed
            ? "repeating-linear-gradient(45deg, var(--checker-a) 0 4px, var(--mixed-hatch) 4px 8px)"
            : cssColor(rgb, opacity),
          boxShadow: "inset 0 0 0 1px #fff",
        }}
      />
      {off && !mixed && (
        <svg aria-hidden viewBox="0 0 28 28" className="absolute inset-0 size-full">
          <line x1="2" y1="26" x2="26" y2="2" stroke="#fff" strokeWidth="3" />
          <line x1="2" y1="26" x2="26" y2="2" stroke="var(--no-paint-slash)" strokeWidth="1.5" />
        </svg>
      )}
    </button>
  );
});
