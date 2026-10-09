import { ToggleGroup, type ToggleOption } from "@/components/ui/toggle-group";

interface PaintRowProps {
  /** The section title ("Stroke", "Fill"). */
  title: string;
  /** The accessible name of the group ("Stroke paint"). */
  groupLabel: string;
  options: readonly ToggleOption<"none" | "solid">[];
  /** `"on"`, `"off"`, or `"mixed"` (nothing pressed). */
  paint: "on" | "off" | "mixed";
  onChange: (on: boolean) => void;
  onReturnFocus: () => void;
  /** The group is the panel's first focus target (`Shift+Ctrl+F`). */
  firstFocus?: boolean;
}

/**
 * The row that opens a section (`specs/0017-style-panel-rework` criteria 5, 6,
 * UX notes section 1): the section title in the label column and the None /
 * Solid group beside it. With Paint None this row is the whole section, and it
 * never moves when the rows below leave or come back, so a mouse press on the
 * switch stays under the pointer.
 */
export function PaintRow({
  title,
  groupLabel,
  options,
  paint,
  onChange,
  onReturnFocus,
  firstFocus = false,
}: PaintRowProps) {
  return (
    <div className="flex h-7 items-center gap-2">
      <h3 className="w-[60px] shrink-0 text-xs font-semibold text-[var(--toolbar-icon)]">{title}</h3>
      <ToggleGroup
        label={groupLabel}
        options={options}
        value={paint === "on" ? "solid" : paint === "off" ? "none" : null}
        onChange={(value) => onChange(value === "solid")}
        itemWidth={44}
        onReturnFocus={onReturnFocus}
        firstFocus={firstFocus}
      />
    </div>
  );
}
