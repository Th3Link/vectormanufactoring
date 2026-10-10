import { FillSection } from "@/components/panel/FillSection";
import { StrokeSection } from "@/components/panel/StrokeSection";
import type { StylePanelApi } from "@/hooks/useStylePanel";

interface StyleSectionProps {
  panel: StylePanelApi;
  onReturnFocus: () => void;
}

/**
 * The "Style" section of the properties panel (`specs/0017-style-panel-
 * rework`, `docs/design-system.md`, "Properties panel: Style section"): the
 * stroke and the fill (the subject line is in the panel's header row, since
 * `0043`). Every value comes from the session; every
 * control sends one command back.
 */
export function StyleSection({ panel, onReturnFocus }: StyleSectionProps) {
  return (
    <div className="flex flex-col gap-2">
      <StrokeSection panel={panel} onReturnFocus={onReturnFocus} />
      <div
        aria-hidden
        className="my-1 h-px"
        style={{ background: "color-mix(in srgb, var(--toolbar-icon) 25%, transparent)" }}
      />
      <FillSection panel={panel} onReturnFocus={onReturnFocus} />
    </div>
  );
}
