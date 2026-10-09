// The buttons of the Style section's toggle groups: names, tooltips and glyphs
// (`docs/design-system.md`, "Properties panel: Style section").

import {
  CapIcon,
  DashSample,
  JoinIcon,
  MarkerIcon,
  NoPaintIcon,
  SolidPaintIcon,
} from "@/components/panel/StyleIcons";
import type { ToggleOption } from "@/components/ui/toggle-group";
import type {
  CapName,
  DashName,
  JoinName,
  MarkerPlaceName,
  MarkerShapeName,
  MarkerSlotName,
} from "@/hooks/useStylePanel";

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

export const DASH_OPTIONS: readonly ToggleOption<DashName>[] = [
  { value: "solid", label: "Solid", tooltip: "Solid", icon: <DashSample name="solid" /> },
  { value: "dash", label: "Dash", tooltip: "Dash: 6 4", icon: <DashSample name="dash" /> },
  { value: "dot", label: "Dot", tooltip: "Dot: 1 3", icon: <DashSample name="dot" /> },
  {
    value: "dash-dot",
    label: "Dash-dot",
    tooltip: "Dash-dot: 6 3 1 3",
    icon: <DashSample name="dash-dot" />,
  },
];

/** The None / Arrow / Dot choices of one marker slot. */
export function markerOptions(slot: MarkerSlotName): readonly ToggleOption<MarkerShapeName>[] {
  const side = slot === "start" ? "Points away from the path. " : "";
  return [
    {
      value: "none",
      label: "None",
      tooltip: "No marker",
      icon: <MarkerIcon slot={slot} shape="none" />,
    },
    {
      value: "arrow",
      label: "Arrow",
      tooltip: `Arrow: a filled triangle, 4 x the stroke width long. ${side}`.trim(),
      icon: <MarkerIcon slot={slot} shape="arrow" />,
    },
    {
      value: "dot",
      label: "Dot",
      tooltip: "Dot: a filled circle, 3 x the stroke width across",
      icon: <MarkerIcon slot={slot} shape="dot" />,
    },
  ];
}

export const PLACE_OPTIONS: readonly ToggleOption<MarkerPlaceName>[] = [
  {
    value: "spaced",
    label: "Spaced",
    tooltip: "Evenly spaced along the whole path",
    icon: <span className="text-sm">Spaced</span>,
  },
  {
    value: "nodes",
    label: "At nodes",
    tooltip: "On every node between the ends. Count is not used",
    icon: <span className="text-sm">At nodes</span>,
  },
];
