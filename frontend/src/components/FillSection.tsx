import { ColorAlphaPicker } from "@/components/ColorAlphaPicker";
import { FILL_OPTIONS } from "@/components/styleOptions";
import { ToggleGroup } from "@/components/ui/toggle-group";
import { colourPanelOf } from "@/lib/colourPanel";
import type { StylePanelApi } from "@/hooks/useStylePanel";

interface FillSectionProps {
  panel: StylePanelApi;
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * The fill rows: the Paint group (No fill, Solid fill; several objects in
 * different states press no item) and, for a solid fill, the colour and opacity.
 */
export function FillSection({ panel, closeKey, onReturnFocus }: FillSectionProps) {
  const { view } = panel;
  const paint = view.fillPaint === "on" ? "solid" : view.fillPaint === "off" ? "none" : null;
  return (
    <>
      <h3 className="mt-2 text-xs font-semibold text-[var(--toolbar-icon)]">Fill</h3>
      <ToggleGroup
        label="Fill paint"
        options={FILL_OPTIONS}
        value={paint}
        onChange={(value) => panel.setFillPaint(value === "solid")}
        disabled={!view.enabled}
        itemWidth={44}
        itemHeight={32}
        onReturnFocus={onReturnFocus}
      />
      {view.fillPaint === "on" && (
        <ColorAlphaPicker
          label="Color"
          name="Fill"
          rgb={view.fillColor}
          rgbMixed={view.fillColorMixed}
          opacity={view.fillOpacity}
          opacityMixed={view.fillOpacityMixed}
          disabled={!view.enabled}
          colorField="fill-color"
          opacityField="fill-opacity"
          panel={colourPanelOf(panel)}
          closeKey={closeKey}
          onReturnFocus={onReturnFocus}
        />
      )}
    </>
  );
}
