import { ColorAlphaPicker } from "@/components/ColorAlphaPicker";
import { FILL_OPTIONS } from "@/components/styleOptions";
import { ToggleGroup } from "@/components/ui/toggle-group";
import type { StylePanelApi } from "@/hooks/useStylePanel";

interface FillSectionProps {
  panel: StylePanelApi;
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * The fill rows: the Fill type group (None or Solid until the gradient modes
 * arrive; a stored gradient reads as no item pressed) and, for a solid fill,
 * the colour and opacity.
 */
export function FillSection({ panel, closeKey, onReturnFocus }: FillSectionProps) {
  const { view } = panel;
  const mode = view.fillMode === "none" || view.fillMode === "solid" ? view.fillMode : null;
  return (
    <>
      <h3 className="mt-2 text-xs font-semibold text-[var(--toolbar-icon)]">Fill</h3>
      <ToggleGroup
        label="Fill type"
        options={FILL_OPTIONS}
        value={mode}
        onChange={(value) => panel.setFillMode(value)}
        disabled={!view.enabled}
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
          disabled={!view.enabled}
          colorField="fill-color"
          opacityField="fill-opacity"
          panel={panel}
          closeKey={closeKey}
          onReturnFocus={onReturnFocus}
        />
      )}
    </>
  );
}
