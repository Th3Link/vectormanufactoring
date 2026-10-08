import { FillSection } from "@/components/FillSection";
import { StrokeSection } from "@/components/StrokeSection";
import type { StylePanelApi } from "@/hooks/useStylePanel";

interface StyleSectionProps {
  panel: StylePanelApi;
  /** Changes when the edited objects or the tool change: closes a popover. */
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * The "Style" section of the properties panel (`specs/0007-stroke-and-fill-
 * styling`, `docs/design-system.md`, "Properties panel: Style section"): the
 * heading, the subject line that says what is being edited, then the stroke and
 * the fill rows. Every value comes from the session; every control sends one
 * command back.
 */
export function StyleSection({ panel, closeKey, onReturnFocus }: StyleSectionProps) {
  return (
    <section aria-labelledby="style-heading" className="flex flex-col gap-2">
      <div>
        <h2 id="style-heading" className="text-sm font-semibold text-[var(--toolbar-icon)]">
          Style
        </h2>
        <p className="mt-0.5 text-xs text-[var(--panel-muted-fg)]">{panel.view.subject}</p>
      </div>
      <StrokeSection panel={panel} closeKey={closeKey} onReturnFocus={onReturnFocus} />
      <div
        aria-hidden
        className="mt-2 h-px"
        style={{ background: "color-mix(in srgb, var(--toolbar-icon) 25%, transparent)" }}
      />
      <FillSection panel={panel} closeKey={closeKey} onReturnFocus={onReturnFocus} />
    </section>
  );
}
