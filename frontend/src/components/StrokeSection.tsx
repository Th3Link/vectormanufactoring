import type { ReactElement } from "react";

import { ColorAlphaPicker } from "@/components/ColorAlphaPicker";
import { DashSelect } from "@/components/DashSelect";
import { NumberField } from "@/components/NumberField";
import { StyleRow } from "@/components/StyleRow";
import { CAP_OPTIONS, JOIN_OPTIONS, PAINT_OPTIONS } from "@/components/styleOptions";
import { ToggleGroup } from "@/components/ui/toggle-group";
import { Tooltip } from "@/components/ui/tooltip";
import { colourPanelOf } from "@/lib/colourPanel";
import type { StylePanelApi } from "@/hooks/useStylePanel";
import { widthText } from "@/lib/styleColor";

const WIDTH_MESSAGES = { width: "Enter a number from 0 to 1000" } as const;

/** Says why a control is disabled. A disabled element gets no pointer events,
 * so the hint hangs on a wrapper and the control inside stops catching them. */
function WhyDisabled({
  reason,
  wide = false,
  children,
}: {
  reason: string | null;
  wide?: boolean;
  children: ReactElement;
}) {
  if (!reason) {
    return children;
  }
  return (
    <Tooltip side="left" content={reason}>
      <span className={`inline-flex [&>*]:pointer-events-none ${wide ? "w-full" : ""}`}>
        {children}
      </span>
    </Tooltip>
  );
}

interface StrokeSectionProps {
  panel: StylePanelApi;
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * The stroke rows: Paint, Color, Width, Dash, Join, Cap. With nothing to edit
 * every control is disabled and shows the frozen defaults; with every edited
 * stroke off, Dash, Join and Cap are disabled because they would have no
 * visible effect, while Color and Width stay enabled (an edit turns the stroke
 * back on).
 */
export function StrokeSection({ panel, closeKey, onReturnFocus }: StrokeSectionProps) {
  const { view } = panel;
  const off = !view.enabled;
  const rowsOff = off || view.strokeAllOff;
  const reason = view.enabled && view.strokeAllOff ? "Stroke is off. Turn it on to change this." : null;
  const paint = view.strokePaint === "on" ? "solid" : view.strokePaint === "off" ? "none" : null;

  return (
    <>
      <h3 className="mt-2 text-xs font-semibold text-[var(--toolbar-icon)]">Stroke</h3>
      <StyleRow label="Paint">
        <ToggleGroup
          label="Stroke paint"
          options={PAINT_OPTIONS}
          value={paint}
          onChange={(value) => panel.setStrokePaint(value === "solid")}
          disabled={off}
          itemWidth={44}
          onReturnFocus={onReturnFocus}
          firstFocus
        />
      </StyleRow>
      <ColorAlphaPicker
        label="Color"
        name="Stroke"
        rgb={view.strokeColor}
        rgbMixed={view.strokeColorMixed}
        opacity={view.strokeOpacity}
        opacityMixed={view.strokeOpacityMixed}
        off={view.strokePaint === "off"}
        disabled={off}
        colorField="stroke-color"
        opacityField="stroke-opacity"
        panel={colourPanelOf(panel)}
        closeKey={closeKey}
        onReturnFocus={onReturnFocus}
      />
      <StyleRow label="Width">
        <NumberField
          label="Stroke width, millimetres"
          shown={widthText(view.strokeWidth)}
          mixed={view.strokeWidthMixed}
          suffix="mm"
          width={96}
          disabled={off}
          onSubmit={(text) => panel.setText("stroke-width", text)}
          messages={WIDTH_MESSAGES}
          onReturnFocus={onReturnFocus}
        />
      </StyleRow>
      <StyleRow label="Dash">
        <WhyDisabled reason={reason} wide>
          <DashSelect
            value={view.strokeDash}
            disabled={rowsOff}
            onChange={panel.setStrokeDash}
            onReturnFocus={onReturnFocus}
          />
        </WhyDisabled>
      </StyleRow>
      <StyleRow label="Join">
        <WhyDisabled reason={reason}>
          <ToggleGroup
            label="Stroke join"
            options={JOIN_OPTIONS}
            value={view.strokeJoin === "mixed" ? null : view.strokeJoin}
            onChange={panel.setStrokeJoin}
            disabled={rowsOff}
            itemWidth={40}
            onReturnFocus={onReturnFocus}
          />
        </WhyDisabled>
      </StyleRow>
      <StyleRow label="Cap">
        <WhyDisabled reason={reason}>
          <ToggleGroup
            label="Stroke cap"
            options={CAP_OPTIONS}
            value={view.strokeCap === "mixed" ? null : view.strokeCap}
            onChange={panel.setStrokeCap}
            disabled={rowsOff}
            itemWidth={40}
            onReturnFocus={onReturnFocus}
          />
        </WhyDisabled>
      </StyleRow>
    </>
  );
}
