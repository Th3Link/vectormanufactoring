import type { KeyboardEvent, PointerEvent } from "react";

import type { BarStop, StopRow, StylePanelApi } from "@/hooks/useStylePanel";
import { cssColor, percentText, toHex } from "@/lib/styleColor";

interface StopThumbProps {
  panel: StylePanelApi;
  stop: BarStop;
  rank: number;
  count: number;
  row: StopRow | undefined;
  selected: boolean;
  canRemove: boolean;
  onPress: (event: PointerEvent<HTMLElement>, rank: number, position: number) => void;
}

/**
 * One thumb under the gradient bar (`docs/design-system.md`, "Gradient bar"): a
 * `role="slider"` pin in the stop's colour. Arrows step 1 %, Shift 10 %, Home
 * and End go to 0 and 100 %, Delete or Backspace removes the stop; none of them
 * reaches the canvas, where Delete would delete the object.
 */
export function StopThumb({
  panel,
  stop,
  rank,
  count,
  row,
  selected,
  canRemove,
  onPress,
}: StopThumbProps) {
  const onKey = (event: KeyboardEvent<HTMLElement>) => {
    const step = event.shiftKey ? 10 : 1;
    const base = row?.position ?? stop.position;
    const next =
      event.key === "ArrowRight" || event.key === "ArrowUp"
        ? base + step
        : event.key === "ArrowLeft" || event.key === "ArrowDown"
          ? base - step
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
    } else if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      event.stopPropagation();
      if (canRemove && panel.removeStop(rank)) {
        // The thumb unmounts: focus goes to its neighbour (the same rank, or the
        // last), so the keyboard does not fall to the page.
        const bar = event.currentTarget.parentElement;
        window.setTimeout(() => {
          const thumbs = bar?.querySelectorAll<HTMLElement>('[role="slider"]');
          thumbs?.[Math.min(rank, thumbs.length - 1)]?.focus();
        }, 0);
      }
    }
  };
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label={`Stop ${rank + 1} of ${count} position`}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(stop.position * 10) / 10}
      aria-valuetext={`${percentText(stop.position)}%, ${toHex(stop.color)}`}
      onPointerDown={(event) => onPress(event, rank, stop.position)}
      onKeyDown={onKey}
      onFocus={() => !selected && panel.selectStop(rank)}
      className={`absolute top-0 h-4 w-3 -translate-x-1/2 cursor-pointer rounded-b-[3px] border-[1.5px] border-[var(--toolbar-icon)] outline-none focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] ${
        selected ? "z-10 ring-2 ring-[var(--editor-accent)] outline outline-2 outline-white" : ""
      }`}
      style={{ left: `${stop.position}%`, background: cssColor(stop.color, stop.opacity) }}
    />
  );
}
