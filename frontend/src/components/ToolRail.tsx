import {
  Circle as CircleIcon,
  MousePointer2,
  PenTool as PenToolIcon,
  Square,
  Star,
} from "lucide-react";
import { Tooltip } from "radix-ui";

import type { Tool } from "@/hooks/useEditorSession";

interface ToolRailProps {
  tool: Tool;
  onSelect: (tool: Tool) => void;
}

interface ToolButtonProps {
  tool: Tool;
  active: boolean;
  label: string;
  shortcut: string;
  icon: React.ReactNode;
  onSelect: (tool: Tool) => void;
}

/**
 * One tool rail button: a real focusable control, tab-reachable, with
 * an `aria-label` and a hover tooltip naming the shortcut
 * (`specification.md`'s UX notes, "Tool rail and tool switching").
 */
function ToolButton({ tool, active, label, shortcut, icon, onSelect }: ToolButtonProps) {
  return (
    <Tooltip.Root delayDuration={400}>
      <Tooltip.Trigger asChild>
        <button
          type="button"
          aria-label={`${label} (${shortcut})`}
          aria-pressed={active}
          onClick={() => onSelect(tool)}
          className="flex size-10 items-center justify-center rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring"
          style={{
            background: active ? "var(--toolbar-icon-active-bg)" : "transparent",
            color: active ? "var(--toolbar-icon-active-fg)" : "var(--toolbar-icon)",
          }}
        >
          {icon}
        </button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          side="right"
          sideOffset={6}
          className="z-50 rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground ring-1 ring-foreground/10"
        >
          {label} ({shortcut})
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

/**
 * The tool rail (`specification.md`'s UX notes, "Tool rail and tool
 * switching"): a 48px vertical rail, docked to the left, Pen then Node,
 * then Rectangle/Ellipse/Polygon-star
 * (`specs/primitive-shapes/specification.md`'s own "the order the maker
 * reaches for them most") — later tools append below, this slice
 * doesn't reorder for them.
 */
export function ToolRail({ tool, onSelect }: ToolRailProps) {
  return (
    <Tooltip.Provider>
      <div
        className="flex w-12 shrink-0 flex-col items-center gap-1 py-1"
        style={{ background: "var(--toolbar-bg)" }}
      >
        <ToolButton
          tool="pen"
          active={tool === "pen"}
          label="Pen tool"
          shortcut="B"
          icon={<PenToolIcon size={20} />}
          onSelect={onSelect}
        />
        <ToolButton
          tool="node"
          active={tool === "node"}
          label="Node tool"
          shortcut="N"
          icon={<MousePointer2 size={20} />}
          onSelect={onSelect}
        />
        <ToolButton
          tool="rectangle"
          active={tool === "rectangle"}
          label="Rectangle tool"
          shortcut="R"
          icon={<Square size={20} />}
          onSelect={onSelect}
        />
        <ToolButton
          tool="ellipse"
          active={tool === "ellipse"}
          label="Ellipse tool"
          shortcut="E"
          icon={<CircleIcon size={20} />}
          onSelect={onSelect}
        />
        <ToolButton
          tool="polygon-star"
          active={tool === "polygon-star"}
          label="Polygon/star tool"
          shortcut="*"
          icon={<Star size={20} />}
          onSelect={onSelect}
        />
      </div>
    </Tooltip.Provider>
  );
}
