import type { PointerEvent } from "react";

import type { StylePanelApi } from "@/hooks/useStylePanel";

/**
 * A press on a gradient thumb: selects the stop and starts a drag that previews
 * its position on every pointer move (see `usePreviewGesture` for the release,
 * which commits once). The move listener is on the window, not the thumb: the
 * thumbs are keyed by rank and a drag past a neighbour changes the rank, which
 * would otherwise lose the pointer. `fractionAt` maps a client x to 0..1 along
 * the bar.
 */
export function useThumbDrag(
  panel: StylePanelApi,
  fractionAt: (clientX: number) => number,
): (event: PointerEvent<HTMLElement>, rank: number) => void {
  return (event, rank) => {
    event.preventDefault();
    event.stopPropagation();
    panel.selectStop(rank);
    event.currentTarget.focus();
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
    // A press without movement still previews the stop where it was pressed, so
    // the gesture ends with its one release.
    move(event.nativeEvent);
  };
}
