import { useLayoutEffect, useRef, useState } from "react";

import type { NudgeFeedback } from "@/hooks/useNudgeFeedback";
import { placeMoveChip } from "@/lib/readoutPlacement";

interface NudgeReadoutProps {
  feedback: Pick<NudgeFeedback, "readout" | "announcement">;
  containerRef: React.RefObject<HTMLDivElement | null>;
}

/**
 * The move readout of a keyboard nudge and its hidden announcement
 * (`specs/0044-editing-quick-wins/` criterion 9): the chip of the "Live transform readout"
 * surface, 16 px right of and below the selection box centre like the typed-move chip, ignoring
 * the pointer and hidden from assistive technology; the live region beside it says the step once
 * it has ended.
 */
export function NudgeReadout({ feedback, containerRef }: NudgeReadoutProps) {
  const { readout, announcement } = feedback;
  const chipRef = useRef<HTMLDivElement>(null);
  const [sizes, setSizes] = useState({
    chip: { width: 0, height: 0 },
    canvas: { width: Number.POSITIVE_INFINITY, height: Number.POSITIVE_INFINITY },
  });

  useLayoutEffect(() => {
    const chip = chipRef.current;
    const container = containerRef.current;
    if (!chip || !container) {
      return;
    }
    const next = {
      chip: { width: chip.offsetWidth, height: chip.offsetHeight },
      canvas: { width: container.clientWidth, height: container.clientHeight },
    };
    setSizes((previous) =>
      previous.chip.width === next.chip.width &&
      previous.chip.height === next.chip.height &&
      previous.canvas.width === next.canvas.width &&
      previous.canvas.height === next.canvas.height
        ? previous
        : next,
    );
  }, [readout, containerRef]);

  const placement = readout ? placeMoveChip(readout, sizes.chip, sizes.canvas) : null;
  return (
    <>
      <div role="status" aria-live="polite" className="sr-only">
        {announcement}
      </div>
      {readout && placement && (
        <div
          ref={chipRef}
          aria-hidden="true"
          className="pointer-events-none absolute z-40 rounded-md px-1.5 py-0.5 text-xs whitespace-nowrap tabular-nums"
          style={{
            left: placement.left,
            top: placement.top,
            background: "var(--toolbar-bg)",
            color: "var(--toolbar-icon)",
          }}
        >
          {readout.text}
        </div>
      )}
    </>
  );
}
