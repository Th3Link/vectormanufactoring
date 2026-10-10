import { useRef } from "react";

import { ColourBlock } from "@/components/panel/ColourBlock";
import { ColourPicker } from "@/components/panel/ColourPicker";
import { PaintRow } from "@/components/panel/PaintRow";
import { FILL_OPTIONS } from "@/components/panel/styleOptions";
import { ValueField } from "@/components/panel/ValueField";
import { useRowsFocus } from "@/components/panel/useRowsFocus";
import type { StylePanelApi } from "@/hooks/useStylePanel";

const PERCENT_MESSAGES = { percent: "Enter a number from 0 to 100" } as const;

interface FillSectionProps {
  panel: StylePanelApi;
  onReturnFocus: () => void;
}

/**
 * The fill section (`specs/0017-style-panel-rework` criteria 6, 15): the Fill
 * title with its None / Solid switch on one row, and under it Color, the
 * picker and Opacity, which are not in the tree while every edited fill is
 * off. A fill colour or opacity edit never turns a fill on; only the switch does.
 */
export function FillSection({ panel, onReturnFocus }: FillSectionProps) {
  const { view } = panel;
  const section = useRef<HTMLDivElement>(null);
  const rowsFocus = useRowsFocus(view.fillRows, section);
  return (
    <div ref={section} className="flex flex-col gap-2">
      <PaintRow
        title="Fill"
        groupLabel="Fill paint"
        options={FILL_OPTIONS}
        paint={view.fillPaint}
        onChange={panel.setFillPaint}
        onReturnFocus={onReturnFocus}
      />
      {view.fillRows && (
        <div className="contents" {...rowsFocus}>
          <ColourBlock
            name="Fill"
            field="fill-color"
            rgb={view.fillColor}
            hex={view.fillHex}
            hexMixed={view.fillHexMixed}
            opacity={view.fillOpacity}
            panel={panel}
            onReturnFocus={onReturnFocus}
          />
          <ColourPicker
            name="Fill"
            field="fill-color"
            rgb={view.fillColor}
            mixed={view.fillColorMixed}
            panel={panel}
          />
          <ValueField
            label="Opacity"
            name="Fill opacity"
            field="fill-opacity"
            text={view.fillOpacityText}
            bar={view.fillOpacityBar}
            mixed={view.fillOpacityMixed}
            resettable={view.fillOpacityResettable}
            unit="%"
            valueNow={view.fillOpacity}
            unitWords="percent"
            typedMax={view.opacityTypedMax}
            defaultText={view.opacityDefaultText}
            messages={PERCENT_MESSAGES}
            panel={panel}
            onReturnFocus={onReturnFocus}
          />
        </div>
      )}
    </div>
  );
}
