import { useRef } from "react";

import { DashSample } from "@/components/StyleIcons";
import { Select, SelectContent, SelectItem, SelectTrigger } from "@/components/ui/select";
import { Tooltip } from "@/components/ui/tooltip";
import type { DashName } from "@/hooks/useStylePanel";

const CHOICES: readonly { value: DashName; label: string }[] = [
  { value: "solid", label: "Solid" },
  { value: "dash", label: "Dash" },
  { value: "dot", label: "Dot" },
  { value: "dash-dot", label: "Dash-dot" },
];

interface DashSelectProps {
  value: DashName | "custom" | "mixed";
  disabled: boolean;
  onChange: (name: DashName) => void;
  onReturnFocus: () => void;
}

/**
 * The stroke dash pattern select (`docs/design-system.md`, "Select (dash)"):
 * the line sample and the name in the trigger and every row. A stored pattern
 * that is none of the presets reads "Custom" (shown, not a choice); several
 * objects with different patterns read "Mixed".
 */
export function DashSelect({ value, disabled, onChange, onReturnFocus }: DashSelectProps) {
  const openedByPointer = useRef(false);
  const current = value === "mixed" ? undefined : value;
  const label =
    value === "custom" ? "Custom" : (CHOICES.find((c) => c.value === value)?.label ?? "");
  return (
    <Select
      value={current}
      disabled={disabled}
      onValueChange={(next) => onChange(next as DashName)}
    >
      <Tooltip content="Patterns scale with the stroke width and draw solid when too small to see">
        <SelectTrigger
          aria-label="Stroke dash pattern"
          placeholder="Mixed"
          onPointerDown={() => {
            openedByPointer.current = true;
          }}
          onKeyDown={() => {
            openedByPointer.current = false;
          }}
        >
          {current && (
            <span className="flex items-center gap-2">
              <DashSample name={current} />
              {label}
            </span>
          )}
        </SelectTrigger>
      </Tooltip>
      <SelectContent
        onCloseAutoFocus={(event) => {
          if (openedByPointer.current) {
            event.preventDefault();
            onReturnFocus();
          }
        }}
      >
        {CHOICES.map((choice) => (
          <SelectItem key={choice.value} value={choice.value}>
            <span className="flex items-center gap-2">
              <DashSample name={choice.value} />
              {choice.label}
            </span>
          </SelectItem>
        ))}
        {value === "custom" && (
          <SelectItem value="custom" disabled>
            <span className="flex items-center gap-2">
              <DashSample name="custom" />
              Custom
            </span>
          </SelectItem>
        )}
      </SelectContent>
    </Select>
  );
}
