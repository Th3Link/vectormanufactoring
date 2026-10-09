import { EntryField } from "@/components/panel/EntryField";
import { StyleRow } from "@/components/panel/StyleRow";
import { DASH_OPTIONS } from "@/components/panel/styleOptions";
import { ToggleGroup } from "@/components/ui/toggle-group";
import { Tooltip } from "@/components/ui/tooltip";
import type { DashName, StylePanelApi } from "@/hooks/useStylePanel";

const PATTERN_MESSAGES = {
  dash: "Enter 1 to 16 numbers from 0 to 1000, for example 1 2 4 2",
} as const;

const PATTERN_TOOLTIP =
  "Lengths in multiples of the stroke width: on, off, on, off. Example: 1 2 4 2. Patterns scale with the stroke width and draw solid when too small to see.";

interface DashRowsProps {
  panel: StylePanelApi;
  onReturnFocus: () => void;
}

/**
 * The Dash and Pattern rows (`specs/0017-style-panel-rework` criteria 28 to
 * 33): four preset buttons in one group and a text line that shows the same
 * stored list as numbers. A list that equals no preset presses no button.
 */
export function DashRows({ panel, onReturnFocus }: DashRowsProps) {
  const { view } = panel;
  const pressed: DashName | null =
    view.strokeDash === "none" || view.strokeDash === "mixed" ? null : view.strokeDash;
  return (
    <>
      <StyleRow label="Dash">
        <ToggleGroup
          label="Stroke dash"
          options={DASH_OPTIONS}
          value={pressed}
          onChange={panel.setStrokeDash}
          itemWidth={44}
          onReturnFocus={onReturnFocus}
        />
      </StyleRow>
      <StyleRow label="Pattern">
        <Tooltip side="left" content={PATTERN_TOOLTIP}>
          <div>
            <EntryField
              label="Stroke dash pattern, multiples of the stroke width, on then off"
              shown={view.strokeDashText}
              mixed={view.strokeDash === "mixed"}
              suffix="x width"
              gutter={56}
              smallSuffix
              placeholder="Solid. Example: 6 4"
              width={176}
              align="left"
              onSubmit={panel.setStrokeDashText}
              messages={PATTERN_MESSAGES}
              onReturnFocus={onReturnFocus}
            />
          </div>
        </Tooltip>
      </StyleRow>
    </>
  );
}
