import type { PointerEvent } from "react";

import type { StylePanelApi } from "@/hooks/useStylePanel";

/** How far the pointer must travel, px, before a press on a thumb is a drag.
 * A press that stays within it only selects the stop: it writes nothing. */
const DRAG_THRESHOLD_PX = 2;

/**
 * A press on a gradient thumb: selects the stop and starts a drag that, once the
 * pointer has moved past a small threshold, previews its position on every move
 * (see `usePreviewGesture` for the release, which commits once). The stop keeps
 * the offset between the pointer and its own position at the press, so grabbing
 * a thumb off its centre does not move it. Only the primary button drags. The
 * move listener is on the window, not the thumb: the thumbs are keyed by rank
 * and a drag past a neighbour changes the rank, which would otherwise lose the
 * pointer. `fractionAt` maps a client x to 0..1 along the bar.
 */
export function useThumbDrag(
  panel: StylePanelApi,
  fractionAt: (clientX: number) => number,
): (event: PointerEvent<HTMLElement>, rank: number, position: number) => void {
  return (event, rank, position) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    panel.selectStop(rank);
    event.currentTarget.focus();
    const startX = event.clientX;
    const grabOffset = fractionAt(startX) * 100 - position;
    let dragging = false;
    const move = (moveEvent: globalThis.PointerEvent) => {
      if (!dragging && Math.abs(moveEvent.clientX - startX) < DRAG_THRESHOLD_PX) {
        return;
      }
      dragging = true;
      panel.previewStop(rank, "position", fractionAt(moveEvent.clientX) * 100 - grabOffset);
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop, true);
      window.removeEventListener("pointercancel", stop, true);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop, true);
    window.addEventListener("pointercancel", stop, true);
  };
}
