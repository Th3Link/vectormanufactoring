import { ColorAlphaPicker } from "@/components/ColorAlphaPicker";
import { GradientEditor } from "@/components/GradientEditor";
import { FILL_OPTIONS } from "@/components/styleOptions";
import { ToggleGroup } from "@/components/ui/toggle-group";
import { colourPanelOf } from "@/hooks/useStylePanel";
import type { StylePanelApi } from "@/hooks/useStylePanel";

interface FillSectionProps {
  panel: StylePanelApi;
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * The fill rows: the Fill type group (None, Solid, Linear, Radial; several
 * objects in different modes press no item), for a solid fill the colour and
 * opacity, and for a gradient the stop editor.
 */
export function FillSection({ panel, closeKey, onReturnFocus }: FillSectionProps) {
  const { view } = panel;
  const mode = view.fillMode === "mixed" ? null : view.fillMode;
  return (
    <>
      <h3 className="mt-2 text-xs font-semibold text-[var(--toolbar-icon)]">Fill</h3>
      <ToggleGroup
        label="Fill type"
        options={FILL_OPTIONS}
        value={mode}
        onChange={panel.setFillMode}
        disabled={!view.enabled}
        itemWidth={44}
        itemHeight={32}
        onReturnFocus={onReturnFocus}
      />
      {view.fillMode === "solid" && (
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
      <GradientEditor panel={panel} closeKey={closeKey} onReturnFocus={onReturnFocus} />
    </>
  );
}
