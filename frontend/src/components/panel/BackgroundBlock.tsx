import { useId, useRef } from "react";

import { ColourBlock } from "@/components/panel/ColourBlock";
import { ColourPicker } from "@/components/panel/ColourPicker";
import { NoPaintIcon, SolidPaintIcon } from "@/components/panel/StyleIcons";
import { ValueField } from "@/components/panel/ValueField";
import { useRowsFocus } from "@/components/panel/useRowsFocus";
import { ToggleGroup, type ToggleOption } from "@/components/ui/toggle-group";
import type { BackgroundPanelApi } from "@/hooks/useBackgroundPanel";

const PERCENT_MESSAGES = { percent: "Enter a number from 0 to 100" } as const;

const PAINT_TOOLTIP = "Document fill. Not an object.";
const PAINT_DESCRIPTION =
  "Fill of the whole document. None shows a checkerboard. The background is not an object; it cannot be selected or moved.";

const PAINT_OPTIONS: readonly ToggleOption<"none" | "solid">[] = [
  { value: "none", label: "None", tooltip: PAINT_TOOLTIP, icon: <NoPaintIcon /> },
  { value: "solid", label: "Solid", tooltip: PAINT_TOOLTIP, icon: <SolidPaintIcon /> },
];

interface BackgroundBlockProps {
  background: BackgroundPanelApi;
  onReturnFocus: () => void;
}

/**
 * The Background block of the Document section (`specs/0040-document-background`
 * criteria 15 to 27, `docs/design-system.md`, row "Background block"): the title
 * "Background" at the left of its row with the None / Solid group at the right,
 * and under it the rows of the Fill section, Color, the picker and Opacity, which
 * are not in the tree while the paint is None. The colour is remembered, nothing
 * is disabled, and the title row never moves.
 */
export function BackgroundBlock({ background, onReturnFocus }: BackgroundBlockProps) {
  const { view } = background;
  const titleId = useId();
  const section = useRef<HTMLDivElement>(null);
  const solid = view.paint === "solid";
  const rowsFocus = useRowsFocus(solid, section);
  return (
    <div ref={section} className="flex flex-col gap-2">
      <div className="flex h-7 items-center justify-between">
        <h3 id={titleId} className="text-xs font-semibold text-[var(--toolbar-icon)]">
          Background
        </h3>
        <ToggleGroup
          label="Background"
          description={PAINT_DESCRIPTION}
          options={PAINT_OPTIONS}
          value={view.paint}
          onChange={background.setPaint}
          itemWidth={44}
          onReturnFocus={onReturnFocus}
        />
      </div>
      {solid && (
        <div className="contents" {...rowsFocus}>
          <ColourBlock
            name="Background"
            rgb={view.color}
            hex={view.hex}
            hexMixed={false}
            opacity={view.opacity}
            picking={view.picking}
            onPick={background.beginPick}
            onSubmitHex={background.setHex}
            onReturnFocus={onReturnFocus}
          />
          <ColourPicker
            name="Background"
            rgb={view.color}
            mixed={false}
            hsvOf={background.hsvOf}
            rgbOf={background.rgbOf}
            onPreview={background.previewHsv}
          />
          <ValueField
            label="Opacity"
            name="Background opacity"
            text={view.opacityText}
            bar={view.opacityBar}
            mixed={false}
            resettable={view.opacityResettable}
            unit="%"
            valueNow={view.opacity}
            unitWords="percent"
            typedMax={view.opacityTypedMax}
            defaultText={view.opacityDefaultText}
            messages={PERCENT_MESSAGES}
            onPreview={background.previewOpacity}
            onStep={background.stepOpacity}
            onReset={background.resetOpacity}
            onSubmit={background.setOpacityText}
            onReturnFocus={onReturnFocus}
          />
        </div>
      )}
    </div>
  );
}
