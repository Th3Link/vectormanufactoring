import { X } from "lucide-react";

import { ColorAlphaPicker } from "@/components/ColorAlphaPicker";
import { NumberField } from "@/components/NumberField";
import { Tooltip } from "@/components/ui/tooltip";
import { stopColourPanel } from "@/hooks/useStylePanel";
import type { StylePanelApi } from "@/hooks/useStylePanel";

const POSITION_MESSAGES = { percent: "Enter a number from 0 to 100" } as const;

/** A position with one decimal at most ("12.5"). */
function positionText(percent: number): string {
  return String(Math.round(percent * 10) / 10);
}

interface StopRowsProps {
  panel: StylePanelApi;
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * The stop list (`docs/design-system.md`, "Stop row"): one 28 px row per stop in
 * position order, every value editable in its row (position, colour swatch,
 * hex, opacity) and a remove button. A row is selected by focusing any control
 * in it; a field that differs across several selected gradients reads "Mixed".
 */
export function StopRows({ panel, closeKey, onReturnFocus }: StopRowsProps) {
  const { view } = panel;
  return (
    <ol className="flex flex-col gap-1" aria-label="Gradient stops">
      {view.stopRows.map((row, rank) => (
        <li
          key={rank}
          onFocusCapture={() => rank !== view.selectedStop && panel.selectStop(rank)}
          onPointerDownCapture={() => rank !== view.selectedStop && panel.selectStop(rank)}
        >
          <ColorAlphaPicker
            name={`Stop ${rank + 1}`}
            rgb={row.color}
            rgbMixed={row.colorMixed}
            opacity={row.opacity}
            opacityMixed={row.opacityMixed}
            disabled={false}
            colorField="color"
            opacityField="opacity"
            panel={stopColourPanel(panel, rank)}
            selected={rank === view.selectedStop}
            closeKey={closeKey}
            onReturnFocus={onReturnFocus}
            before={
              <NumberField
                label={`Stop ${rank + 1} position percent`}
                shown={positionText(row.position)}
                mixed={row.positionMixed}
                suffix="%"
                width={52}
                compact
                disabled={false}
                onSubmit={(text) => panel.setStopText(rank, "position", text)}
                messages={POSITION_MESSAGES}
                onReturnFocus={onReturnFocus}
              />
            }
            after={
              view.stopsObjects === 1 ? (
                <Tooltip
                  side="left"
                  content={view.stopsCanRemove ? "Remove stop" : "A gradient keeps at least 2 stops"}
                >
                  <span className="inline-flex">
                    <button
                      type="button"
                      aria-label={`Remove stop ${rank + 1}`}
                      disabled={!view.stopsCanRemove}
                      onClick={(event) => {
                        panel.removeStop(rank);
                        if (event.detail > 0) {
                          onReturnFocus();
                        }
                      }}
                      className="flex size-7 items-center justify-center rounded-[5px] text-[var(--toolbar-icon)] outline-none enabled:hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] disabled:text-[var(--field-disabled-fg)] disabled:opacity-60"
                    >
                      <X size={14} aria-hidden />
                    </button>
                  </span>
                </Tooltip>
              ) : null
            }
          />
        </li>
      ))}
    </ol>
  );
}
