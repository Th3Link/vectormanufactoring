import type { ReactElement } from "react";

import { ColorAlphaPicker } from "@/components/ColorAlphaPicker";
import { DashSelect } from "@/components/DashSelect";
import { NumberField } from "@/components/NumberField";
import { StyleRow } from "@/components/StyleRow";
import { CapIcon, JoinIcon, NoPaintIcon, SolidPaintIcon } from "@/components/StyleIcons";
import { Tooltip } from "@/components/ui/tooltip";
import { ToggleGroup } from "@/components/ui/toggle-group";
import type { ToggleOption } from "@/components/ui/toggle-group";
import type { CapName, JoinName, StylePanelApi } from "@/hooks/useStylePanel";
import { widthText } from "@/lib/styleColor";

const WIDTH_MESSAGES = { width: "Enter a number from 0 to 1000" } as const;

const PAINT_OPTIONS: readonly ToggleOption<"none" | "solid">[] = [
  { value: "none", label: "No stroke", tooltip: "No stroke", icon: <NoPaintIcon /> },
  { value: "solid", label: "Solid stroke", tooltip: "Solid stroke", icon: <SolidPaintIcon /> },
];

const FILL_OPTIONS: readonly ToggleOption<"none" | "solid">[] = [
  { value: "none", label: "No fill", tooltip: "No fill", icon: <NoPaintIcon /> },
  { value: "solid", label: "Solid fill", tooltip: "Solid fill", icon: <SolidPaintIcon /> },
];

const JOIN_OPTIONS: readonly ToggleOption<JoinName>[] = [
  {
    value: "miter",
    label: "Miter join",
    tooltip: "Miter join: a sharp point; becomes a bevel at very sharp angles",
    icon: <JoinIcon join="miter" />,
  },
  { value: "round", label: "Round join", tooltip: "Round join", icon: <JoinIcon join="round" /> },
  {
    value: "bevel",
    label: "Bevel join",
    tooltip: "Bevel join: a flat cut",
    icon: <JoinIcon join="bevel" />,
  },
];

const CAP_OPTIONS: readonly ToggleOption<CapName>[] = [
  {
    value: "butt",
    label: "Butt cap",
    tooltip: "Butt cap: ends at the endpoint",
    icon: <CapIcon cap="butt" />,
  },
  { value: "round", label: "Round cap", tooltip: "Round cap", icon: <CapIcon cap="round" /> },
  {
    value: "square",
    label: "Square cap",
    tooltip: "Square cap: extends half the width past the endpoint",
    icon: <CapIcon cap="square" />,
  },
];

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
    <Tooltip content={reason}>
      <span className={`inline-flex [&>*]:pointer-events-none ${wide ? "w-full" : ""}`}>{children}</span>
    </Tooltip>
  );
}

interface StyleSectionProps {
  panel: StylePanelApi;
  /** Changes when the edited objects or the tool change: closes a popover. */
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * The "Style" section of the properties panel (`specs/0007-stroke-and-fill-
 * styling`, `docs/design-system.md`, "Properties panel: Style section"):
 * stroke (Paint, Color, Width, Dash, Join, Cap) and fill (None or Solid, and
 * the colour). Every value comes from the session; every control sends one
 * command back. With nothing to edit every control is disabled and shows the
 * frozen defaults; with every edited stroke off, Dash, Join and Cap are
 * disabled because they would have no visible effect.
 */
export function StyleSection({ panel, closeKey, onReturnFocus }: StyleSectionProps) {
  const { view } = panel;
  const off = !view.enabled;
  const strokeOff = off || view.strokeAllOff;
  const strokeOffReason =
    view.enabled && view.strokeAllOff ? "Stroke is off. Turn it on to change this." : null;
  const paintValue =
    view.strokePaint === "on" ? "solid" : view.strokePaint === "off" ? "none" : null;
  const fillValue = view.fillMode === "none" || view.fillMode === "solid" ? view.fillMode : null;

  return (
    <section aria-labelledby="style-heading" className="flex flex-col gap-2">
      <div>
        <h2 id="style-heading" className="text-sm font-semibold text-[var(--toolbar-icon)]">
          Style
        </h2>
        <p className="mt-0.5 text-xs text-[var(--panel-muted-fg)]">{view.subject}</p>
      </div>

      <h3 className="mt-2 text-xs font-semibold text-[var(--toolbar-icon)]">Stroke</h3>
      <StyleRow label="Paint">
        <ToggleGroup
          label="Stroke paint"
          options={PAINT_OPTIONS}
          value={paintValue}
          onChange={(value) => panel.setStrokePaint(value === "solid")}
          disabled={off}
          itemWidth={44}
          onReturnFocus={onReturnFocus}
          firstFocus
        />
      </StyleRow>
      <ColorAlphaPicker
          name="Stroke"
          rgb={view.strokeColor}
          rgbMixed={view.strokeColorMixed}
          opacity={view.strokeOpacity}
          opacityMixed={view.strokeOpacityMixed}
          off={view.strokePaint === "off"}
          disabled={off}
          colorField="stroke-color"
          opacityField="stroke-opacity"
          panel={panel}
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
        <WhyDisabled reason={strokeOffReason} wide>
          <DashSelect
            value={view.strokeDash}
            disabled={strokeOff}
            onChange={panel.setStrokeDash}
            onReturnFocus={onReturnFocus}
          />
        </WhyDisabled>
      </StyleRow>
      <StyleRow label="Join">
        <WhyDisabled reason={strokeOffReason}>
        <ToggleGroup
          label="Stroke join"
          options={JOIN_OPTIONS}
          value={view.strokeJoin === "mixed" ? null : view.strokeJoin}
          onChange={panel.setStrokeJoin}
          disabled={strokeOff}
          itemWidth={40}
          onReturnFocus={onReturnFocus}
        />
        </WhyDisabled>
      </StyleRow>
      <StyleRow label="Cap">
        <WhyDisabled reason={strokeOffReason}>
        <ToggleGroup
          label="Stroke cap"
          options={CAP_OPTIONS}
          value={view.strokeCap === "mixed" ? null : view.strokeCap}
          onChange={panel.setStrokeCap}
          disabled={strokeOff}
          itemWidth={40}
          onReturnFocus={onReturnFocus}
        />
        </WhyDisabled>
      </StyleRow>

      <div
        aria-hidden
        className="mt-2 h-px"
        style={{ background: "color-mix(in srgb, var(--toolbar-icon) 25%, transparent)" }}
      />
      <h3 className="mt-2 text-xs font-semibold text-[var(--toolbar-icon)]">Fill</h3>
      <ToggleGroup
        label="Fill type"
        options={FILL_OPTIONS}
        value={fillValue}
        onChange={(value) => panel.setFillMode(value)}
        disabled={off}
        itemWidth={44}
        itemHeight={32}
        onReturnFocus={onReturnFocus}
      />
      {view.fillMode === "solid" && (
        <ColorAlphaPicker
            name="Fill"
            rgb={view.fillColor}
            rgbMixed={view.fillColorMixed}
            opacity={view.fillOpacity}
            opacityMixed={view.fillOpacityMixed}
            disabled={off}
            colorField="fill-color"
            opacityField="fill-opacity"
            panel={panel}
            closeKey={closeKey}
            onReturnFocus={onReturnFocus}
          />
      )}
    </section>
  );
}
