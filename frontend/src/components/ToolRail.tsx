import {
  Circle as CircleIcon,
  MousePointer2,
  MousePointer as SelectIcon,
  PenTool as PenToolIcon,
  Square,
  Star,
} from "lucide-react";
import { Tooltip } from "radix-ui";

import type { Tool } from "@/hooks/useEditorSession";

interface ToolRailProps {
  tool: Tool;
  /** How many objects are selected; with the Select tool active and one or
   * more selected the plain letters R and S act on the selection, so the
   * Rectangle tool is reached by "Esc, R" (`specs/edit-interaction-polish/`
   * criterion 62). */
  selectionCount: number;
  onSelect: (tool: Tool) => void;
  /** Called after a mouse click on a button, so the tool letters keep working
   * (keyboard activation keeps focus on the rail for Tab navigation). */
  onReturnFocus: () => void;
}

interface ToolButtonProps {
  tool: Tool;
  active: boolean;
  label: string;
  shortcut: string;
  icon: React.ReactNode;
  onSelect: (tool: Tool) => void;
  onReturnFocus: () => void;
}

/**
 * One tool rail button: a real focusable control, tab-reachable, with
 * an `aria-label` and a hover tooltip naming the shortcut
 * (`specification.md`'s UX notes, "Tool rail and tool switching").
 */
function ToolButton({
  tool,
  active,
  label,
  shortcut,
  icon,
  onSelect,
  onReturnFocus,
}: ToolButtonProps) {
  return (
    <Tooltip.Root delayDuration={400}>
      <Tooltip.Trigger asChild>
        <button
          type="button"
          aria-label={`${label} (${shortcut})`}
          aria-pressed={active}
          onClick={(event) => {
            onSelect(tool);
            // `detail` is 0 for keyboard activation, 1 or more for a mouse click.
            if (event.detail > 0) {
              onReturnFocus();
            }
          }}
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
 * The tool rail (`docs/design-system.md`'s 2026-10-05 "Left tool panel":
 * floats *over* the canvas, inset 12px from the left and top edges,
 * elevated with `--panel-elevation-shadow`, fixed 48px icon-column
 * width — the canvas is never resized or repositioned to make room for
 * it). Select is first — a deliberate, one-time exception to "new tools
 * append in ship order" (acceptance criterion 12; `docs/design-system.md`:
 * "the rail's new default/master tool"), then Pen, Node, Rectangle,
 * Ellipse, Polygon-star, each shifted down one slot but otherwise in
 * their own established order.
 */
export function ToolRail({ tool, selectionCount, onSelect, onReturnFocus }: ToolRailProps) {
  // The one state that changes a letter: the Select tool with a selection.
  const letterActsOnSelection = tool === "select" && selectionCount > 0;
  return (
    <Tooltip.Provider>
      <div
        className="absolute top-3 left-3 z-20 flex w-12 flex-col items-center gap-1 rounded-lg py-1"
        style={{
          background: "var(--toolbar-bg)",
          boxShadow: "var(--panel-elevation-shadow)",
        }}
      >
        <ToolButton
          tool="select"
          active={tool === "select"}
          label="Select tool"
          shortcut="S or Esc"
          icon={<SelectIcon size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="pen"
          active={tool === "pen"}
          label="Pen tool"
          shortcut="B"
          icon={<PenToolIcon size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="node"
          active={tool === "node"}
          label="Node tool"
          shortcut="N"
          icon={<MousePointer2 size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="rectangle"
          active={tool === "rectangle"}
          label="Rectangle tool"
          shortcut={letterActsOnSelection ? "Esc, R" : "R"}
          icon={<Square size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="ellipse"
          active={tool === "ellipse"}
          label="Ellipse tool"
          shortcut="E"
          icon={<CircleIcon size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="polygon-star"
          active={tool === "polygon-star"}
          label="Polygon/star tool"
          shortcut="*"
          icon={<Star size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
      </div>
    </Tooltip.Provider>
  );
}
