import { useRef } from "react";

import { ColourBlock } from "@/components/panel/ColourBlock";
import { ColourPicker } from "@/components/panel/ColourPicker";
import { DashRows } from "@/components/panel/DashRows";
import { MarkerRows } from "@/components/panel/MarkerRows";
import { PaintRow } from "@/components/panel/PaintRow";
import { StyleRow } from "@/components/panel/StyleRow";
import { CAP_OPTIONS, JOIN_OPTIONS, PAINT_OPTIONS } from "@/components/panel/styleOptions";
import { ValueField } from "@/components/panel/ValueField";
import { useRowsFocus } from "@/components/panel/useRowsFocus";
import { ToggleGroup } from "@/components/ui/toggle-group";
import type { StylePanelApi } from "@/hooks/useStylePanel";

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
  const markersFocus = useRowsFocus(view.markersShown, section);
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
          <ValueField
            label="Opacity"
            name="Stroke opacity"
            field="stroke-opacity"
            text={view.strokeOpacityText}
            bar={view.strokeOpacityBar}
            mixed={view.strokeOpacityMixed}
            resettable={view.strokeOpacityResettable}
            unit="%"
            valueNow={view.strokeOpacity}
            unitWords="percent"
            typedMax={100}
            defaultText="100 %"
            messages={PERCENT_MESSAGES}
            panel={panel}
            onReturnFocus={onReturnFocus}
          />
          <ValueField
            label="Width"
            name="Stroke width"
            field="stroke-width"
            text={view.strokeWidthText}
            bar={view.strokeWidthBar}
            mixed={view.strokeWidthMixed}
            resettable={view.strokeWidthResettable}
            unit="mm"
            valueNow={view.strokeWidth}
            unitWords="millimetres"
            typedMax={1000}
            defaultText="0.25 mm"
            messages={WIDTH_MESSAGES}
            panel={panel}
            onReturnFocus={onReturnFocus}
          />
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
          {view.markersShown && (
            <div className="contents" {...markersFocus}>
              <MarkerRows panel={panel} onReturnFocus={onReturnFocus} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
