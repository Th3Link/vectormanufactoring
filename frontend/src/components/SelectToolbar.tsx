import { ScaleStrokeSwitch } from "@/components/ScaleStrokeSwitch";

export interface SelectToolbarProps {
  scaleStrokeWidth: boolean;
  onSetScaleStrokeWidth: (on: boolean) => void;
}

/**
 * The Select tool's own permanent contextual bar (`docs/design-system.md`,
 * "Switch" -> "Host and bar layout"; `specs/0005-object-transform/
 * specification.md` criterion 30): shown whenever the Select tool is
 * active, with or without a selection, in the same overlay slot and with
 * the same look as `NodeToolbar` and `ShapeToolbar`. It holds the "Scale
 * stroke width" switch; later Select-tool settings are appended after it.
 */
export function SelectToolbar({
  scaleStrokeWidth,
  onSetScaleStrokeWidth,
}: SelectToolbarProps) {
  return (
    <div
      className="flex pointer-events-auto h-9 min-w-0 items-center gap-3 rounded-lg px-2 text-sm"
      style={{
        background: "var(--toolbar-bg)",
        boxShadow: "var(--panel-elevation-shadow)",
      }}
    >
      <ScaleStrokeSwitch
        checked={scaleStrokeWidth}
        onCheckedChange={onSetScaleStrokeWidth}
      />
    </div>
  );
}
