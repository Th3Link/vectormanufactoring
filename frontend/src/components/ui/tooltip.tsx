import { Tooltip as TooltipPrimitive } from "radix-ui";
import type { ReactElement, ReactNode } from "react";

/** One provider for a group of tooltips (`docs/design-system.md`, "Frontend
 * components to add"). */
export const TooltipProvider = TooltipPrimitive.Provider;

interface TooltipProps {
  /** The text after a 400 ms rest. */
  content: ReactNode;
  /** Which side opens; the panel's tooltips open left, over the canvas. */
  side?: "top" | "right" | "bottom" | "left";
  /** The trigger: one element that takes a ref. */
  children: ReactElement;
}

/**
 * A tooltip in the house style: 400 ms, `bg-popover`, rendered in a portal so
 * it may cover the canvas but never the panel row it belongs to.
 */
export function Tooltip({ content, side = "left", children }: TooltipProps) {
  return (
    <TooltipPrimitive.Root delayDuration={400}>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content
          side={side}
          sideOffset={8}
          className="z-[60] max-w-64 rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground ring-1 ring-foreground/10"
        >
          {content}
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}
