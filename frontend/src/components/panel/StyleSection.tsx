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
 * heading with the subject line that says what is being edited on one row,
 * then the stroke and the fill. Every value comes from the session; every
 * control sends one command back.
 */
export function StyleSection({ panel, onReturnFocus }: StyleSectionProps) {
  return (
    <section aria-labelledby="style-heading" className="flex flex-col gap-2">
      <div className="flex h-6 items-center justify-between gap-2">
        <h2 id="style-heading" className="text-sm font-semibold text-[var(--toolbar-icon)]">
          Style
        </h2>
        <p className="truncate text-xs text-[var(--panel-muted-fg)]">{panel.view.subject}</p>
      </div>
      <StrokeSection panel={panel} onReturnFocus={onReturnFocus} />
      <div
        aria-hidden
        className="my-1 h-px"
        style={{ background: "color-mix(in srgb, var(--toolbar-icon) 25%, transparent)" }}
      />
      <FillSection panel={panel} onReturnFocus={onReturnFocus} />
    </section>
  );
}
