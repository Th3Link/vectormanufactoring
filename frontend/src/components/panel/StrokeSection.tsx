import { useRef } from "react";

import { ColourBlock } from "@/components/panel/ColourBlock";
import { ColourPicker } from "@/components/panel/ColourPicker";
import { DashRows } from "@/components/panel/DashRows";
import { EntryField } from "@/components/panel/EntryField";
import { PaintRow } from "@/components/panel/PaintRow";
import { StyleRow } from "@/components/panel/StyleRow";
import { CAP_OPTIONS, JOIN_OPTIONS, PAINT_OPTIONS } from "@/components/panel/styleOptions";
import { useRowsFocus } from "@/components/panel/useRowsFocus";
import { ToggleGroup } from "@/components/ui/toggle-group";
import type { StylePanelApi } from "@/hooks/useStylePanel";
import { percentText, widthText } from "@/lib/styleColor";

const WIDTH_MESSAGES = { width: "Enter a number from 0 to 1000" } as const;
const PERCENT_MESSAGES = { percent: "Enter a number from 0 to 100" } as const;

interface StrokeSectionProps {
  panel: StylePanelApi;
  onReturnFocus: () => void;
}

/**
 * The stroke section (`specs/0017-style-panel-rework` criteria 5 to 10): the
 * Stroke title with its Paint switch on one row, and under it Color, the
 * picker, Opacity, Width, Dash, Pattern, Join and Cap. While every edited
 * stroke is off the rows are not in the tree at all; with some on and some off
 * they show their stored values.
 */
export function StrokeSection({ panel, onReturnFocus }: StrokeSectionProps) {
  const { view } = panel;
  const section = useRef<HTMLDivElement>(null);
  const rowsFocus = useRowsFocus(view.strokeRows, section);
  return (
    <div ref={section} className="flex flex-col gap-2">
      <PaintRow
        title="Stroke"
        groupLabel="Stroke paint"
        options={PAINT_OPTIONS}
        paint={view.strokePaint}
        onChange={panel.setStrokePaint}
        onReturnFocus={onReturnFocus}
        firstFocus
      />
      {view.strokeRows && (
        <div className="contents" {...rowsFocus}>
          <ColourBlock
            name="Stroke"
            field="stroke-color"
            rgb={view.strokeColor}
            hex={view.strokeHex}
            hexMixed={view.strokeHexMixed}
            opacity={view.strokeOpacity}
            panel={panel}
            onReturnFocus={onReturnFocus}
          />
          <ColourPicker
            name="Stroke"
            field="stroke-color"
            rgb={view.strokeColor}
            mixed={view.strokeColorMixed}
            panel={panel}
          />
          <StyleRow label="Opacity">
            <EntryField
              label="Stroke opacity percent"
              shown={percentText(view.strokeOpacity)}
              mixed={view.strokeOpacityMixed}
              suffix="%"
              width={176}
              onSubmit={(text) => panel.setText("stroke-opacity", text)}
              messages={PERCENT_MESSAGES}
              onReturnFocus={onReturnFocus}
            />
          </StyleRow>
          <StyleRow label="Width">
            <EntryField
              label="Stroke width, millimetres"
              shown={widthText(view.strokeWidth)}
              mixed={view.strokeWidthMixed}
              suffix="mm"
              width={176}
              onSubmit={(text) => panel.setText("stroke-width", text)}
              messages={WIDTH_MESSAGES}
              onReturnFocus={onReturnFocus}
            />
          </StyleRow>
          <DashRows panel={panel} onReturnFocus={onReturnFocus} />
          <StyleRow label="Join">
            <ToggleGroup
              label="Stroke join"
              options={JOIN_OPTIONS}
              value={view.strokeJoin === "mixed" ? null : view.strokeJoin}
              onChange={panel.setStrokeJoin}
              itemWidth={40}
              onReturnFocus={onReturnFocus}
            />
          </StyleRow>
          <StyleRow label="Cap">
            <ToggleGroup
              label="Stroke cap"
              options={CAP_OPTIONS}
              value={view.strokeCap === "mixed" ? null : view.strokeCap}
              onChange={panel.setStrokeCap}
              itemWidth={40}
              onReturnFocus={onReturnFocus}
            />
          </StyleRow>
        </div>
      )}
    </div>
  );
}
