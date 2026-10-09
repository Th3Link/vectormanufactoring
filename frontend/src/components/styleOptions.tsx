// The buttons of the Style section's toggle groups: names, tooltips and glyphs
// (`docs/design-system.md`, "Properties panel: Style section").

import {
  CapIcon,
  JoinIcon,
  NoPaintIcon,
  SolidPaintIcon,
} from "@/components/StyleIcons";
import type { ToggleOption } from "@/components/ui/toggle-group";
import type { CapName, JoinName } from "@/hooks/useStylePanel";

export const PAINT_OPTIONS: readonly ToggleOption<"none" | "solid">[] = [
  { value: "none", label: "No stroke", tooltip: "No stroke", icon: <NoPaintIcon /> },
  { value: "solid", label: "Solid stroke", tooltip: "Solid stroke", icon: <SolidPaintIcon /> },
];

export const FILL_OPTIONS: readonly ToggleOption<"none" | "solid">[] = [
  { value: "none", label: "No fill", tooltip: "No fill", icon: <NoPaintIcon /> },
  { value: "solid", label: "Solid fill", tooltip: "Solid fill", icon: <SolidPaintIcon /> },
];

export const JOIN_OPTIONS: readonly ToggleOption<JoinName>[] = [
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

export const CAP_OPTIONS: readonly ToggleOption<CapName>[] = [
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
