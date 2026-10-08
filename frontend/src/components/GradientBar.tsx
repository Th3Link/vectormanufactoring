import { useId, useRef } from "react";
import type { KeyboardEvent, PointerEvent } from "react";

import type { StopRow, StylePanelApi } from "@/hooks/useStylePanel";
import type { BarStop } from "@/hooks/useStylePanel";
import { cssColor, percentText, toHex } from "@/lib/styleColor";

const CHECKER =
  "linear-gradient(45deg, var(--checker-b) 25%, transparent 25%, transparent 75%, var(--checker-b) 75%), linear-gradient(45deg, var(--checker-b) 25%, transparent 25%, transparent 75%, var(--checker-b) 75%)";
const HATCH = "repeating-linear-gradient(45deg, var(--checker-a) 0 4px, var(--mixed-hatch) 4px 8px)";

/** The bar is 244 px wide, 16 px high; the thumbs are 12 x 16 px pins below it. */
const BAR_WIDTH = 244;

interface GradientBarProps {
  panel: StylePanelApi;
  /** The bar's stops in position order; `null` is a neutral hatched track. */
  stops: BarStop[] | null;
  rows: StopRow[];
  selected: number;
  canAdd: boolean;
  canRemove: boolean;
}

/**
 * The gradient bar (`docs/design-system.md`, "Gradient bar"): the ramp drawn
 * as an inline SVG `linearGradient`, which interpolates sRGB without
 * premultiplying as the renderer does (a CSS gradient premultiplies and would
 * show another ramp for stops with opacity), over a checkerboard, and one thumb
 * per stop below it. A drag moves a stop (live preview, one commit on release);
 * a click on the bar away from a thumb adds one; a thumb is a slider (arrows
 * 1 %, Shift 10 %, Home and End, Delete removes). With several objects whose
 * lists differ the bar is a hatched track with no thumbs.
 */
export function GradientBar({
  panel,
  stops,
  rows,
  selected,
  canAdd,
  canRemove,
}: GradientBarProps) {
  const gradientId = useId().replace(/:/g, "");
  const track = useRef<HTMLDivElement>(null);

  const fractionAt = (clientX: number): number => {
    const rect = track.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0) {
      return 0;
    }
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  };

  const startDrag = (event: PointerEvent<HTMLElement>, rank: number) => {
    event.preventDefault();
    event.stopPropagation();
    panel.selectStop(rank);
    (event.currentTarget as HTMLElement).focus();
    const move = (moveEvent: globalThis.PointerEvent) =>
      panel.previewStop(rank, "position", fractionAt(moveEvent.clientX) * 100);
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop, true);
      window.removeEventListener("pointercancel", stop, true);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop, true);
    window.addEventListener("pointercancel", stop, true);
    // A press without movement still previews the stop where it was pressed,
    // so the gesture ends with its one release.
    move(event.nativeEvent);
  };

  const onThumbKey = (event: KeyboardEvent<HTMLElement>, rank: number) => {
    const row = rows[rank];
    if (!row) {
      return;
    }
    const step = event.shiftKey ? 10 : 1;
    const next =
      event.key === "ArrowRight" || event.key === "ArrowUp"
        ? row.position + step
        : event.key === "ArrowLeft" || event.key === "ArrowDown"
          ? row.position - step
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? 100
              : null;
    if (next !== null) {
      event.preventDefault();
      event.stopPropagation();
      panel.selectStop(rank);
      panel.previewStop(rank, "position", Math.min(100, Math.max(0, next)));
    } else if ((event.key === "Delete" || event.key === "Backspace") && canRemove) {
      event.preventDefault();
      event.stopPropagation();
      panel.removeStop(rank);
    } else if (event.key === "Delete" || event.key === "Backspace") {
      // Never reaches the canvas, where it would delete the object.
      event.preventDefault();
      event.stopPropagation();
    }
  };

  const ramp = stops ?? [];
  return (
    <div className="flex flex-col" style={{ width: BAR_WIDTH }}>
      <div
        ref={track}
        data-testid="gradient-bar"
        className="relative h-4 cursor-crosshair rounded-[3px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)]"
        style={{
          backgroundImage: stops ? CHECKER : HATCH,
          backgroundSize: "12px 12px",
          backgroundPosition: "0 0, 6px 6px",
          backgroundColor: "var(--checker-a)",
        }}
        onPointerDown={(event) => {
          // A press on the track (not on a thumb) adds a stop there.
          if (stops && canAdd) {
            panel.addStopAt(fractionAt(event.clientX));
          }
        }}
        aria-hidden={stops ? undefined : true}
      >
        {stops && ramp.length > 0 && (
          <svg
            width="100%"
            height="100%"
            preserveAspectRatio="none"
            className="absolute inset-0"
            aria-hidden
          >
            <defs>
              <linearGradient id={gradientId} x1="0" x2="1" y1="0" y2="0">
                {ramp.map((stop, index) => (
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
      {stops && (
        <div className="relative h-4">
          {ramp.map((stop, rank) => (
            <div
              key={rank}
              role="slider"
              tabIndex={0}
              aria-label={`Stop ${rank + 1} of ${ramp.length} position`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(stop.position * 10) / 10}
              aria-valuetext={`${percentText(stop.position)}%, ${toHex(stop.color)}`}
              onPointerDown={(event) => startDrag(event, rank)}
              onKeyDown={(event) => onThumbKey(event, rank)}
              onFocus={() => rank !== selected && panel.selectStop(rank)}
              className={`absolute top-0 h-4 w-3 -translate-x-1/2 cursor-pointer rounded-b-[3px] border-[1.5px] outline-none focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] ${
                rank === selected
                  ? "z-10 border-[var(--toolbar-icon)] ring-2 ring-[var(--editor-accent)] outline outline-2 outline-white"
                  : "border-[var(--toolbar-icon)]"
              }`}
              style={{
                left: `${stop.position}%`,
                background: cssColor(stop.color, stop.opacity),
              }}
            />
          ))}
        </div>
      )}
    </div>
  );
}
