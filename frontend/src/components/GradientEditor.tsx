import { GradientBar } from "@/components/GradientBar";
import { StopRows } from "@/components/StopRows";
import { Tooltip } from "@/components/ui/tooltip";
import type { StylePanelApi } from "@/hooks/useStylePanel";

interface GradientEditorProps {
  panel: StylePanelApi;
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * The gradient stop editor (`docs/design-system.md`, "Gradient bar", "Stop
 * row", "Add stop", "Info line"): the bar, the list, Add stop and the lines
 * that explain the states. Shown for gradients in one mode; with different stop
 * counts it is replaced by a message (criterion 34); with no stops it says
 * nothing is painted and Add stop is enabled (criterion 35).
 */
export function GradientEditor({ panel, closeKey, onReturnFocus }: GradientEditorProps) {
  const { view } = panel;
  if (view.stopsState === "different-counts") {
    return (
      <p className="text-xs text-[var(--panel-muted-fg)]">
        Selected gradients have different numbers of stops.
      </p>
    );
  }
  if (view.stopsState !== "editor") {
    return null;
  }
  const single = view.stopsObjects === 1;
  return (
    <div className="flex flex-col gap-2">
      <GradientBar
        panel={panel}
        stops={view.stopsBarShown ? view.stopBar : null}
        rows={view.stopRows}
        selected={view.selectedStop}
        canAdd={view.stopsCanAdd}
        canRemove={view.stopsCanRemove}
      />
      {view.stopRows.length === 0 && (
        <p className="text-xs text-[var(--panel-muted-fg)]">
          No stops. Nothing is painted. Add a stop.
        </p>
      )}
      <StopRows panel={panel} closeKey={closeKey} onReturnFocus={onReturnFocus} />
      {single && (
        <Tooltip
          side="left"
          content={
            view.stopsCanAdd ? "Add a stop in the widest gap" : "A gradient holds at most 16 stops"
          }
        >
          <span className="flex">
            <button
              type="button"
              disabled={!view.stopsCanAdd}
              onClick={(event) => {
                panel.addStop();
                if (event.detail > 0) {
                  onReturnFocus();
                }
              }}
              className="h-7 w-full rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] bg-white text-sm text-[var(--toolbar-icon)] outline-none enabled:hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--toolbar-bg)] disabled:border-transparent disabled:bg-[var(--field-disabled-bg)] disabled:text-[var(--field-disabled-fg)]"
            >
              Add stop
            </button>
          </span>
        </Tooltip>
      )}
      {view.stopsBoxNote && (
        <p className="text-xs text-[var(--panel-muted-fg)]">
          Gradient spans the shape&apos;s selection box, which is the square around a polygon or
          star.
        </p>
      )}
    </div>
  );
}
