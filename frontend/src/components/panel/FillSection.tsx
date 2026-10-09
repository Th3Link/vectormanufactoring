import { useRef } from "react";

import { ColourBlock } from "@/components/panel/ColourBlock";
import { ColourPicker } from "@/components/panel/ColourPicker";
import { EntryField } from "@/components/panel/EntryField";
import { PaintRow } from "@/components/panel/PaintRow";
import { StyleRow } from "@/components/panel/StyleRow";
import { FILL_OPTIONS } from "@/components/panel/styleOptions";
import { useRowsFocus } from "@/components/panel/useRowsFocus";
import type { StylePanelApi } from "@/hooks/useStylePanel";
import { percentText } from "@/lib/styleColor";

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
          <StyleRow label="Opacity">
            <EntryField
              label="Fill opacity percent"
              shown={percentText(view.fillOpacity)}
              mixed={view.fillOpacityMixed}
              suffix="%"
              width={176}
              onSubmit={(text) => panel.setText("fill-opacity", text)}
              messages={PERCENT_MESSAGES}
              onReturnFocus={onReturnFocus}
            />
          </StyleRow>
        </div>
      )}
    </div>
  );
}
