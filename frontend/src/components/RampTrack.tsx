import { forwardRef, useId } from "react";

import type { BarStop } from "@/hooks/useStylePanel";
import { cssColor } from "@/lib/styleColor";

const CHECKER =
  "linear-gradient(45deg, var(--checker-b) 25%, transparent 25%, transparent 75%, var(--checker-b) 75%), linear-gradient(45deg, var(--checker-b) 25%, transparent 25%, transparent 75%, var(--checker-b) 75%)";
const HATCH = "repeating-linear-gradient(45deg, var(--checker-a) 0 4px, var(--mixed-hatch) 4px 8px)";

interface RampTrackProps {
  /** The stops in position order; `null` is a neutral hatched track. */
  stops: BarStop[] | null;
  /** A press on the track (not a thumb) at `clientX`. */
  onPress: (clientX: number) => void;
}

/**
 * The bar itself (`docs/design-system.md`, "Gradient bar"): the ramp drawn as an
 * inline SVG `linearGradient`, which interpolates sRGB without premultiplying as
 * the renderer does (a CSS gradient premultiplies and would show another ramp
 * for stops with opacity), over a checkerboard. With no stops it is an empty
 * checkerboard; with several objects whose lists differ, a hatched track.
 */
export const RampTrack = forwardRef<HTMLDivElement, RampTrackProps>(function RampTrack(
  { stops, onPress },
  ref,
) {
  const gradientId = useId().replace(/:/g, "");
  return (
    <div
      ref={ref}
      data-testid="gradient-bar"
      className="relative h-4 cursor-crosshair rounded-[3px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)]"
      style={{
        backgroundImage: stops ? CHECKER : HATCH,
        backgroundSize: "12px 12px",
        backgroundPosition: "0 0, 6px 6px",
        backgroundColor: "var(--checker-a)",
      }}
      onPointerDown={(event) => onPress(event.clientX)}
      aria-hidden={stops ? undefined : true}
    >
      {stops && stops.length > 0 && (
        <svg width="100%" height="100%" preserveAspectRatio="none" className="absolute inset-0" aria-hidden>
          <defs>
            <linearGradient id={gradientId} x1="0" x2="1" y1="0" y2="0">
              {stops.map((stop, index) => (
                <stop
                  key={index}
                  offset={stop.position / 100}
                  stopColor={cssColor(stop.color, 100)}
                  stopOpacity={stop.opacity / 100}
                />
              ))}
            </linearGradient>
          </defs>
          <rect width="100%" height="100%" fill={`url(#${gradientId})`} />
        </svg>
      )}
    </div>
  );
});
