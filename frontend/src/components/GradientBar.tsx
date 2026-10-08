import { useRef } from "react";

import { RampTrack } from "@/components/RampTrack";
import { StopThumb } from "@/components/StopThumb";
import { Tooltip } from "@/components/ui/tooltip";
import type { BarStop, StopRow, StylePanelApi } from "@/hooks/useStylePanel";
import { useThumbDrag } from "@/hooks/useThumbDrag";

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
 * The gradient bar and its thumbs: a press on the track away from a thumb adds a
 * stop there; a thumb drags (live preview, one commit on release) and steps with
 * the keyboard. With several objects whose lists differ there are no thumbs.
 */
export function GradientBar({
  panel,
  stops,
  rows,
  selected,
  canAdd,
  canRemove,
}: GradientBarProps) {
  const track = useRef<HTMLDivElement>(null);
  const fractionAt = (clientX: number): number => {
    const rect = track.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0) {
      return 0;
    }
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  };
  const press = useThumbDrag(panel, fractionAt);
  const full = stops !== null && stops.length >= 16;

  const rampTrack = (
    <RampTrack
      ref={track}
      stops={stops}
      blocked={full}
      onPress={(clientX) => stops && canAdd && panel.addStopAt(fractionAt(clientX))}
    />
  );

  return (
    <div className="flex flex-col" style={{ width: BAR_WIDTH }}>
      {full ? (
        <Tooltip side="left" content="A gradient holds at most 16 stops">
          <span className="block">{rampTrack}</span>
        </Tooltip>
      ) : (
        rampTrack
      )}
      {stops && (
        <div className="relative h-4">
          {stops.map((stop, rank) => (
            <StopThumb
              key={rank}
              panel={panel}
              stop={stop}
              rank={rank}
              count={stops.length}
              row={rows[rank]}
              selected={rank === selected}
              canRemove={canRemove}
              onPress={press}
            />
          ))}
        </div>
      )}
    </div>
  );
}
